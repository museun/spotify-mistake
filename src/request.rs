use std::sync::Arc;

use librespot::{core::FileId, metadata::Track};

use crate::{spotify_lyrics::SpotifyLyrics, twitch};

#[derive(Clone)]
pub struct Request {
    pub track: Arc<Track>,
    pub id: uuid::Uuid,
    pub image_id: Option<FileId>,
    pub user: twitch::User,
    pub lyrics: SpotifyLyrics,
    pub added_on: time::OffsetDateTime,
}
