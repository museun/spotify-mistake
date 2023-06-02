use librespot::{core::SpotifyId, playback::player::PlayerEvent};

#[derive(Default, Copy, Clone, Debug)]
pub enum NextPlayingState {
    Playing,
    #[default]
    StopPlaying,
}

// TODO this isn't needed, we can just wrap their state and provide our methods on it
#[derive(Default)]
pub enum PlayerState {
    #[default]
    NotPlaying,
    Loading {
        req_id: u64,
        id: SpotifyId,
    },
    Playing {
        req_id: u64,
        pos: u32,
        id: SpotifyId,
    },
    Seeked {
        req_id: u64,
        id: SpotifyId,
        pos: u32,
    },
    Paused {
        req_id: u64,
        pos: u32,
        id: SpotifyId,
    },
    PreloadNextTrack {
        req_id: u64,
        id: SpotifyId,
    },
    EndOfPlaying {
        req_id: u64,
        id: SpotifyId,
    },
    Unavailable {
        req_id: u64,
        id: SpotifyId,
    },
}

impl PlayerState {
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            Self::NotPlaying => "NotPlaying",
            Self::Loading { .. } => "Loading",
            Self::Playing { .. } => "Playing",
            Self::Seeked { .. } => "Seeked",
            Self::Paused { .. } => "Paused",
            Self::PreloadNextTrack { .. } => "PreloadNextTrack",
            Self::EndOfPlaying { .. } => "EndOfPlaying",
            Self::Unavailable { .. } => "Unavailable",
        }
    }
}

impl PlayerState {
    pub const fn is_not_playing(&self) -> bool {
        matches!(self, Self::NotPlaying { .. })
    }

    pub const fn is_playing(&self) -> bool {
        matches!(self, Self::Playing { .. })
    }

    pub const fn is_paused(&self) -> bool {
        matches!(self, Self::Paused { .. })
    }

    pub const fn is_done_playing(&self) -> bool {
        matches!(self, Self::EndOfPlaying { .. })
    }

    pub fn is_loading(&self, current: &SpotifyId) -> bool {
        matches!(self, Self::Loading { id, .. } if id == current)
    }

    pub const fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable { .. })
    }
}

impl TryFrom<PlayerEvent> for PlayerState {
    type Error = ();

    fn try_from(event: PlayerEvent) -> Result<Self, Self::Error> {
        let ev = match event {
            PlayerEvent::Stopped {
                play_request_id,
                track_id,
            } => Self::NotPlaying,

            PlayerEvent::Loading {
                play_request_id,
                track_id,
                position_ms: _,
            } => Self::Loading {
                req_id: play_request_id,
                id: track_id,
            },

            PlayerEvent::Playing {
                play_request_id,
                track_id,
                position_ms,
            } => Self::Playing {
                req_id: play_request_id,
                pos: position_ms,
                id: track_id,
            },

            PlayerEvent::Paused {
                play_request_id,
                track_id,
                position_ms,
            } => Self::Paused {
                req_id: play_request_id,
                pos: position_ms,
                id: track_id,
            },

            PlayerEvent::EndOfTrack {
                play_request_id,
                track_id,
            } => Self::EndOfPlaying {
                req_id: play_request_id,
                id: track_id,
            },

            PlayerEvent::Unavailable {
                play_request_id,
                track_id,
            } => Self::Unavailable {
                req_id: play_request_id,
                id: track_id,
            },

            PlayerEvent::TimeToPreloadNextTrack {
                track_id: id,
                play_request_id: req_id,
            } => Self::PreloadNextTrack { id, req_id },

            PlayerEvent::Seeked {
                play_request_id,
                track_id,
                position_ms,
            } => Self::Seeked {
                req_id: play_request_id,
                id: track_id,
                pos: position_ms,
            },

            _ => return Err(()),
        };

        Ok(ev)
    }
}
