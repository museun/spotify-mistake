use std::sync::Arc;

use librespot::{core::FileId, metadata::Track};

use crate::{spotify_lyrics::SpotifyLyrics, twitch};

// TODO track when this was added
#[derive(Clone)]
pub struct Request {
    pub track: Arc<Track>,
    pub image_id: Option<FileId>,
    pub user: twitch::User,
    pub lyrics: SpotifyLyrics,
}
