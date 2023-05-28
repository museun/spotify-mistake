use std::time::Duration;

use egui::Color32;
use tokio::{
    io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
};
use twitch_message::{
    encode::{join, ping, pong, privmsg, register, reply, ALL_CAPABILITIES},
    messages::{
        types::{MsgId, Nickname, UserId},
        Privmsg, TwitchMessage,
    },
    IntoStatic, ParseResult,
};

use crate::util::{select2, Either};

#[derive(::serde::Serialize, ::serde::Deserialize, Clone)]
pub struct User {
    pub id: UserId,
    pub name: Nickname,
    pub color: Color32,
}

pub struct Config {
    pub name: String,
    pub pass: String,
    pub channel: String,
}

pub struct Writer {
    sender: UnboundedSender<WriteKind>,
}

impl Writer {
    pub const fn new(sender: UnboundedSender<WriteKind>) -> Self {
        Self { sender }
    }

    pub fn reply(&self, msg_id: impl Into<MsgId>, data: impl ToString) {
        let _ = self.sender.send(WriteKind::Reply {
            id: msg_id.into(),
            data: data.to_string(),
        });
    }

    pub fn say(&self, data: impl ToString) {
        let _ = self.sender.send(WriteKind::Say {
            data: data.to_string(),
        });
    }
}

pub enum WriteKind {
    Reply { id: MsgId, data: String },
    Say { data: String },
}

pub async fn connect(
    config: Config,
    events: UnboundedSender<Privmsg<'static>>,
    mut writer: UnboundedReceiver<WriteKind>,
) {
    let mut success = false;
    'outer: loop {
        if success {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
        success = true;

        log::info!("connecting to Twitch");
        let mut stream =
            match tokio::net::TcpStream::connect(twitch_message::TWITCH_IRC_ADDRESS).await {
                Ok(stream) => stream,
                Err(err) => {
                    log::warn!("cannot connect: {err}");
                    continue;
                }
            };

        let register = register(&config.name, &config.pass, ALL_CAPABILITIES);

        if !write_all(&mut stream, register.to_string()).await {
            log::warn!("cannot register");
            continue;
        }

        let (read, mut write) = stream.split();

        let mut lines = BufReader::new(read).lines();
        let mut our_name = <Option<String>>::default();

        #[derive(Default)]
        enum PingTimeout {
            Waiting {
                token: String,
            },
            #[default]
            Nothing,
        }

        let mut timeout = PingTimeout::default();

        'inner: loop {
            let line = std::pin::pin!(lines.next_line());
            let recv = std::pin::pin!(writer.recv());

            match {
                match tokio::time::timeout(Duration::from_secs(60), select2(line, recv)).await {
                    Ok(ready) => ready,
                    Err(..) if matches!(timeout, PingTimeout::Waiting { .. }) => {
                        log::warn!("timed out, and no ping waiting");
                        continue 'outer;
                    }
                    Err(..) => {
                        let token = std::iter::repeat_with(fastrand::alphanumeric)
                            .take(10)
                            .collect::<String>();

                        timeout = PingTimeout::Waiting {
                            token: token.clone(),
                        };

                        let ping = ping(&token);
                        if write_all(&mut write, ping.to_string()).await {
                            continue 'inner;
                        } else {
                            continue 'outer;
                        }
                    }
                }
            } {
                Either::Left(Ok(Some(line))) => {
                    let message = match twitch_message::parse(&line) {
                        Ok(ParseResult { message, .. }) => message,
                        Err(err) => {
                            log::warn!(
                                "cannot parse message: '{line}': {err}",
                                line = line.escape_debug()
                            );
                            continue 'outer;
                        }
                    };

                    match message.as_enum() {
                        TwitchMessage::Privmsg(msg) => {
                            if events.send(msg.into_static()).is_err() {
                                break 'outer;
                            }
                        }

                        TwitchMessage::Ping(msg) => {
                            let msg = pong(&msg.token);
                            if !write_all(&mut write, msg.to_string()).await {
                                continue 'outer;
                            }
                        }

                        TwitchMessage::Pong(pong) => match &timeout {
                            PingTimeout::Waiting { token } if token == &pong.token => {
                                let _ = std::mem::take(&mut timeout);
                            }
                            PingTimeout::Waiting { .. } => {
                                continue 'outer;
                            }
                            PingTimeout::Nothing => {}
                        },

                        TwitchMessage::Ready(ready) => {
                            let _ = our_name.replace(ready.name.to_string());
                            log::info!("IRC is ready");
                        }

                        TwitchMessage::GlobalUserState(state) => {
                            log::info!("Twitch is ready");
                            log::debug!("joining: {channel}", channel = config.channel);
                            if !write_all(&mut write, join(&config.channel).to_string()).await {
                                continue 'outer;
                            }
                        }
                        _ => {}
                    }
                }

                Either::Right(Some(kind)) => {
                    let data = match kind {
                        WriteKind::Reply { id, data } => {
                            reply(&id, &config.channel, &data).to_string() //
                        }
                        WriteKind::Say { data } => {
                            privmsg(&config.channel, &data).to_string() //
                        }
                    };

                    if !write_all(&mut write, data).await {
                        log::warn!("cannot write");
                        continue 'outer;
                    }
                }

                Either::Left(..) => continue 'outer,
                Either::Right(..) => break 'outer,
            }
        }
    }
}

async fn write_all(
    stream: &mut (impl AsyncWrite + Unpin + Send + Sync),
    data: impl AsRef<[u8]> + Send + Sync,
) -> bool {
    if stream.write_all(data.as_ref()).await.is_ok() {
        return stream.flush().await.is_ok();
    }
    false
}
