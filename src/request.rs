use std::sync::Arc;

use egui::{text::LayoutJob, Color32, CursorIcon, FontId, Label, Sense, TextFormat};

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

// TODO move these elsehwere
impl Request {
    pub fn display_active(
        &self,
        ui: &mut egui::Ui,
        fid: &FontId,
        space: f32,
        active: Color32,
        inactive: Color32,
        user: &twitch::User,
    ) -> egui::Response {
        let mut job = LayoutJob::simple(self.track.name.clone(), fid.clone(), active, 0.0);
        job.append(" by ", space, TextFormat::simple(fid.clone(), inactive));

        for (i, artist) in self.track.artists.iter().enumerate() {
            if i > 0 {
                job.append(", ", 0.0, TextFormat::simple(fid.clone(), inactive));
            }
            job.append(&artist.name, space, TextFormat::simple(fid.clone(), active))
        }

        job.append("(", space, TextFormat::simple(fid.clone(), inactive));

        job.append(
            &format!("{}", user.name),
            0.0,
            TextFormat::simple(fid.clone(), user.color),
        );

        job.append(")", 0.0, TextFormat::simple(fid.clone(), inactive));

        let resp = ui
            .add(Label::new(job).wrap(true))
            .interact(Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);

        if resp.clicked_by(egui::PointerButton::Primary) {
            ui.output_mut(|o| {
                o.open_url(format!(
                    "https://open.spotify.com/track/{id}",
                    id = self.track.id.to_base62().unwrap()
                ))
            })
        }

        resp
    }

    pub fn layout(
        &self,
        ui: &mut egui::Ui,
        fid: &FontId,
        space: f32,
        active: Color32,
        inactive: Color32,
    ) -> egui::Response {
        let mut job = LayoutJob::simple(self.track.name.clone(), fid.clone(), active, 0.0);
        job.append(" by ", space, TextFormat::simple(fid.clone(), inactive));

        for (i, artist) in self.track.artists.iter().enumerate() {
            if i > 0 {
                job.append(", ", 0.0, TextFormat::simple(fid.clone(), inactive));
            }
            job.append(&artist.name, space, TextFormat::simple(fid.clone(), active))
        }

        let resp = ui
            .add(Label::new(job).wrap(true))
            .interact(Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);

        if resp.clicked_by(egui::PointerButton::Primary) {
            ui.output_mut(|o| {
                o.open_url(format!(
                    "https://open.spotify.com/track/{id}",
                    id = self.track.id.to_base62().unwrap()
                ))
            })
        }

        resp
    }
}
