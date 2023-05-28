use std::sync::Arc;

use librespot::metadata::{lyrics::SyncType, Lyrics};

#[derive(Clone)]
pub struct SpotifyLyrics {
    pub lyrics: Arc<[LyricLine]>,
    pub synced: bool,
}

impl Default for SpotifyLyrics {
    fn default() -> Self {
        Self {
            lyrics: Arc::from(vec![]),
            synced: false,
        }
    }
}

impl SpotifyLyrics {
    pub fn fix_up(lyrics: Lyrics, max_dur_millis: usize) -> Self {
        let synced = matches!(lyrics.lyrics.sync_type, SyncType::LineSynced);

        let mut prev = None;
        let mut lines = Vec::with_capacity(lyrics.lyrics.lines.len());

        for line in lyrics.lyrics.lines.into_iter().rev() {
            let start = line.start_time_ms.parse().expect("valid time");
            let end = prev.replace(start).unwrap_or(max_dur_millis);

            lines.push(LyricLine {
                start,
                end,
                data: line.words,
            });
        }

        lines.reverse();
        Self {
            lyrics: Arc::from(lines),
            synced,
        }
    }
}

pub struct LyricLine {
    pub start: usize,
    pub end: usize,
    pub data: String,
}
