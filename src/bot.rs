use std::{borrow::Cow, sync::Arc, time::Instant};

use egui::Color32;
use hashbrown::HashMap;
use librespot::{
    core::{session::Session, spotify_id::SpotifyId},
    metadata::{image::ImageSize, Lyrics, Metadata, Track},
};
use rspotify::{
    model::{Country, Market, Page, SearchResult, SearchType},
    prelude::BaseClient,
    ClientCredsSpotify,
};

use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    oneshot,
};
use twitch_message::messages::{
    types::{MsgId, UserId},
    MsgIdRef, Privmsg, UserIdRef,
};

use crate::{
    ext::JoinWith,
    history,
    spotify_lyrics::SpotifyLyrics,
    twitch::{self, ChannelTarget},
    Request,
};

pub enum SynthEvent<T> {
    Synthetic(T),
    Organic(T),
}

struct SelectionItem {
    name: String,
    artist: String,
    id: String,
}

pub struct Selection {
    items: Vec<SelectionItem>,
    created: Instant,
    msg_id: MsgId,
    offset: usize,
}

pub struct Bot {
    pub config: twitch::Config,
    pub events: UnboundedReceiver<Privmsg<'static>>,
    pub writer: twitch::Writer,
    pub produce: UnboundedSender<SynthEvent<Request>>,
    pub requests: UnboundedSender<oneshot::Sender<Option<Request>>>,
    pub session: Session,
    pub spotify: ClientCredsSpotify,
    pub selection: HashMap<UserId, Selection>,
}

impl Bot {
    pub fn new(
        config: twitch::Config,
        events: UnboundedReceiver<Privmsg<'static>>,
        writer: twitch::Writer,
        produce: UnboundedSender<SynthEvent<Request>>,
        requests: UnboundedSender<oneshot::Sender<Option<Request>>>,
        session: Session,
        spotify: ClientCredsSpotify,
    ) -> Self {
        Self {
            config,
            events,
            writer,
            produce,
            requests,
            session,
            spotify,
            selection: HashMap::new(),
        }
    }

    pub async fn process(mut self) {
        while let Some(msg) = self.events.recv().await {
            let Some(user_id) = msg.user_id() else { continue };
            let msg_id = msg.msg_id().unwrap();

            // TODO clear stale entries

            if self.handle_converstation(&msg, user_id, msg_id).await {
                continue;
            }

            if self.handle_send_title(&msg, msg_id).await {
                continue;
            }

            let Some(req) = msg.data.strip_prefix("~req ") else { continue };

            if let Some(track_id) = Self::try_parse(req, msg_id, &self.writer) {
                self.handle_song_req(&msg, msg_id, track_id).await;
                continue;
            }

            self.handle_search(&msg, req, user_id, msg_id).await;
        }
    }

    // TODO make this parser more dynamic
    // TODO show play stats
    // TODO allow for ~prev
    // TODO allow for aliases
    async fn handle_send_title(&mut self, msg: &Privmsg<'_>, msg_id: &MsgIdRef) -> bool {
        if msg.data != "~song" {
            return false;
        }

        let (tx, rx) = oneshot::channel();
        let _ = self.requests.send(tx);
        let resp = rx.await.expect("gui is running");

        let Some(resp) = resp else {
            self.writer.reply(ChannelTarget::Main, msg_id, "nothing is playing");
            return true
        };

        self.writer.say(
            ChannelTarget::Main,
            format!(
                "{name} by {artist} (requested by {user}) \
                 @ https://open.spotify.com/track/{id}",
                name = resp.track.name,
                artist = resp.track.artists.iter().map(|c| &c.name).join(", "),
                user = resp.user.name,
                id = resp.track.id.to_base62().unwrap(),
            ),
        );

        true
    }

    async fn handle_converstation(
        &mut self,
        msg: &Privmsg<'static>,
        user_id: &UserIdRef,
        msg_id: &MsgIdRef,
    ) -> bool {
        if msg.channel.strip_prefix('#').unwrap_or(&msg.channel) != self.config.spam_channel {
            return false;
        }

        let Some(parent_msg_id) = msg.reply_parent_msg_id() else { return false };

        let mut remove = false;
        if let Some(selection) = self
            .selection
            .get_mut(user_id)
            .filter(|sel| sel.msg_id == parent_msg_id)
        {
            struct Index(usize);
            impl std::str::FromStr for Index {
                type Err = &'static str;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    s.strip_prefix('#')
                        .unwrap_or_else(|| s.trim())
                        .parse()
                        .map(Self)
                        .map_err(|_| "invalid selection")
                }
            }

            enum Reply {
                Add,
                More,
                Select(usize),
            }

            impl std::str::FromStr for Reply {
                type Err = &'static str;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    s.trim()
                        .splitn(2, ' ')
                        .last()
                        .map(|s| match s {
                            "add" => Ok(Self::Add),
                            // TODO allow more things to get around the message limit
                            "more" => Ok(Self::More),
                            s => s
                                .parse()
                                .map(|Index(index)| Self::Select(index.saturating_sub(1))),
                        })
                        .transpose()?
                        .ok_or("invalid selection")
                }
            }

            let reply: Reply = match msg.data.parse() {
                Ok(reply) => reply,
                Err(err) => {
                    self.writer.reply(ChannelTarget::Spam, parent_msg_id, err);
                    return true;
                }
            };

            macro_rules! print_menu {
                () => {
                    for (index, item) in selection
                        .items
                        .iter()
                        .enumerate()
                        .skip(selection.offset)
                        .take(3)
                    {
                        let data = format!(
                            "#{index} {name} by {artist}",
                            index = index + 1,
                            name = item.name,
                            artist = item.artist
                        );
                        self.writer.reply(ChannelTarget::Spam, parent_msg_id, data);
                    }
                    selection.offset += 3;
                };
            }

            let item = match reply {
                Reply::Add => &selection.items[0],
                Reply::More => {
                    print_menu!();
                    return true;
                }
                Reply::Select(index) => {
                    let Some(item) = selection.items.get(index) else {
                            print_menu!();
                            return true;
                        };
                    item
                }
            };

            let spotify_id = SpotifyId::from_uri(&item.id).expect("valid id");

            let item = history::HistoryItem {
                id: uuid::Uuid::new_v4(),
                added_on: time::OffsetDateTime::now_utc(),
                spotify_id,
                user: Cow::Owned(twitch::User {
                    id: user_id.to_owned(),
                    color: msg
                        .color()
                        .map_or(Color32::GRAY, |twitch_message::Color(r, g, b)| {
                            Color32::from_rgb(r, g, b)
                        }),
                    name: msg.sender.clone().into_owned(),
                }),
            };

            let Some(req) = item.lookup(&self.session).await else {
                let data = "cannot look up that item :(";
                self.writer.say(ChannelTarget::Main, data);
                self.writer.reply(ChannelTarget::Spam, parent_msg_id, data);
                return false;
            };

            let data = format!(
                "added {name} by {artist} \
                     @ https://open.spotify.com/track/{id}",
                name = req.track.name,
                artist = req.track.artists.iter().map(|c| &c.name).join(", "),
                id = req.track.id.to_base62().unwrap(),
            );
            self.writer.say(ChannelTarget::Main, &data);
            self.writer.reply(ChannelTarget::Spam, parent_msg_id, data);

            let _ = self.produce.send(SynthEvent::Organic(req));
            remove = true
        }

        if remove {
            self.selection.remove(user_id);
        }

        false
    }

    async fn handle_song_req(&mut self, msg: &Privmsg<'_>, msg_id: &MsgIdRef, track_id: SpotifyId) {
        let Ok(track) = Track::get(&self.session, &track_id).await.map(Arc::new) else {
            return
        };

        let image_id = track
            .album
            .covers
            .iter()
            .find_map(|c| (c.size == ImageSize::DEFAULT).then_some(c.id));

        let lyrics = Lyrics::get(&self.session, &track_id)
            .await
            .ok()
            .map(|lyrics| SpotifyLyrics::fix_up(lyrics, track.duration as _))
            .unwrap_or_default();

        let user = twitch::User {
            id: msg.user_id().unwrap().to_owned(),
            color: msg
                .color()
                .map_or(Color32::GRAY, |twitch_message::Color(r, g, b)| {
                    Color32::from_rgb(r, g, b)
                }),
            name: msg.sender.clone().into_owned(),
        };

        let data = format!(
            "added {name} by {artist}
            @ https://open.spotify.com/track/{id}",
            name = track.name,
            artist = track.artists.iter().map(|c| &c.name).join(", "),
            id = track.id.to_base62().unwrap(),
        );

        self.writer.say(ChannelTarget::Main, &data);
        self.writer.reply(ChannelTarget::Spam, msg_id, &data);

        let _ = self.produce.send(SynthEvent::Organic(Request {
            id: uuid::Uuid::new_v4(),
            added_on: time::OffsetDateTime::now_utc(),
            track,
            image_id,
            user,
            lyrics,
        }));
    }

    fn try_parse(input: &str, msg_id: &MsgIdRef, writer: &twitch::Writer) -> Option<SpotifyId> {
        macro_rules! nope {
            ($msg:expr) => {{
                writer.reply(ChannelTarget::Main, msg_id, $msg);
                return None;
            }};
        }

        let url = url::Url::parse(input).ok()?;

        match url.scheme() {
            "http" | "https" if matches!(url.domain(), Some("open.spotify.com")) => {}
            _ => nope!("only spotify URLs are allowed"),
        };

        if let Some(id) = url
            .path()
            .strip_prefix("/track/")
            .filter(|c| c.len() == 22)
            .map(|id| format!("spotify:track:{id}"))
            .and_then(|id| SpotifyId::from_uri(&id).ok())
        {
            return Some(id);
        }

        nope!("invalid spotify URN")
    }

    async fn handle_search(
        &mut self,
        msg: &Privmsg<'static>,
        req: &str,
        user_id: &UserIdRef,
        msg_id: &MsgIdRef,
    ) {
        let results = match self
            .spotify
            // TODO make this stuff configurable
            .search(
                req,
                SearchType::Track,
                Some(Market::Country(Country::UnitedStates)),
                None,
                Some(9),
                None,
            )
            .await
        {
            Ok(results) => results,
            Err(err) => {
                log::error!("cannot lookup item: {err}");
                self.writer
                    .reply(ChannelTarget::Spam, msg_id, "something went wrong :(");
                return;
            }
        };

        let SearchResult::Tracks(Page { items, .. }) = results else { return };

        self.selection.remove(user_id);
        let selection = self
            .selection
            .entry(user_id.to_owned())
            .or_insert(Selection {
                items: Vec::new(),
                created: Instant::now(),
                msg_id: msg_id.to_owned(),
                offset: 0,
            });

        for (i, item) in items
            .into_iter()
            .filter_map(|item| {
                let playable = item.is_playable.unwrap_or(true);
                let id = item.id?;
                playable.then_some(SelectionItem {
                    name: item.name,
                    artist: item.artists.iter().map(|c| &c.name).join(", "),
                    id: id.to_string(),
                })
            })
            .enumerate()
        {
            // TODO allow things other than 'more' to get around message limit
            const HEADER: &str =
                r#"I found the following, reply with "add" to add it, \
                or "more" to get more."#;

            if i == 0 {
                let data = format!(
                    "{HEADER} {name} by {artist}",
                    name = item.name,
                    artist = item.artist
                );
                self.writer.reply(ChannelTarget::Spam, msg_id, data);
            }

            selection.items.push(item)
        }

        if selection.items.is_empty() {
            self.writer.reply(
                ChannelTarget::Spam,
                msg_id,
                format!("nothing found for: {req}"),
            );
            self.selection.remove(user_id);
        }
    }
}
