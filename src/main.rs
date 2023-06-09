#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]
use std::sync::Arc;

use egui::mutex::Mutex;

use librespot::{
    core::{authentication::Credentials, cache::Cache, session::Session, SessionConfig},
    playback::{
        audio_backend,
        config::{AudioFormat, PlayerConfig},
        player::Player,
    },
};

use tokio::sync::mpsc::{self, unbounded_channel};

mod async_adapter;
mod bot;
mod control;
mod ext;
mod history;
mod image_cache;
mod player_state;
mod progress;
mod request;
mod scrollable;
mod spotify_lyrics;
mod tab_selection;
mod twitch;
mod util;
mod views;
mod volume_state;

mod db;

use crate::{request::Request, volume_state::VolumeState};

const APP_ID: &str = "0D29F111-0601-4C75-901E-5C6341D518B1";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from([".dev.env", ".secrets.env"]);
    alto_logger::init_term_logger().expect("init logger");

    fn get(key: &str) -> anyhow::Result<String> {
        std::env::var(key).map_err(|_| anyhow::anyhow!("`{key}` must be set"))
    }

    let config = twitch::Config {
        name: get("TWITCH_NAME")?,
        pass: get("TWITCH_PASS")?,
        main_channel: get("TWITCH_MAIN_CHANNEL")?,
        spam_channel: get("TWITCH_SPAM_CHANNEL")?,
    };

    let spotify_api_client = rspotify::ClientCredsSpotify::with_config(
        rspotify::Credentials::new(&get("SPOTIFY_CLIENT_ID")?, &get("SPOTIFY_CLIENT_SECRET")?),
        rspotify::Config {
            token_refreshing: true,
            ..rspotify::Config::default()
        },
    );

    tokio::spawn({
        let client = spotify_api_client.clone();
        async move {
            client
                .request_token()
                .await
                .expect("valid spotify client-id/client-secret pair")
        }
    });

    let credentials = Credentials::with_password(
        get("SPOTIFY_USER_NAME")?, //
        get("SPOTIFY_PASS")?,
    );

    let session = Session::new(
        SessionConfig {
            device_id: APP_ID.to_string(),
            ..SessionConfig::default()
        },
        Cache::new(Some("./librespot/"), None, None, None).map(Some)?,
    );

    session.connect(credentials, true).await?;

    let backend = audio_backend::find(None).expect("audio backend enabled");

    let volume = VolumeState {
        volume: Arc::new(Mutex::new(1.0)),
    };

    let player = Player::new(
        PlayerConfig::default(),
        session.clone(),
        Box::new(volume.clone()),
        move || backend(None, AudioFormat::default()),
    );

    let (events_tx, events) = unbounded_channel();
    let (writer, writer_rx) = unbounded_channel();

    let writer = twitch::Writer::new(writer);

    tokio::spawn({
        let config = config.clone();
        async move { twitch::connect(config, events_tx, writer_rx).await }
    });

    let (tx, rx) = mpsc::unbounded_channel();
    let (req_tx, req_rx) = mpsc::unbounded_channel();

    tokio::spawn(
        bot::Bot::new(
            config,
            events,
            writer,
            tx.clone(),
            req_tx,
            session.clone(),
            spotify_api_client,
        )
        .process(),
    );

    eframe::run_native(
        "spotify-mistake",
        eframe::NativeOptions::default(),
        Box::new(|cc| control::Control::create(cc, session, player, volume, tx, rx, req_rx)),
    )
    .unwrap();
    Ok(())
}
