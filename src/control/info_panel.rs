use egui::{vec2, Layout, Rect, Sense, TextStyle};
use librespot::playback::player::Player;

use crate::{
    image_cache::ImageCache,
    request::Request,
    views::{ImageView, RequestView},
};

use super::lyrics_panel::LyricsPanel;

pub struct InfoPanel<'a> {
    pub request: &'a Request,
    pub cache: &'a mut ImageCache,
    pub player: &'a Player,
    pub elapsed: Option<usize>,
    pub height: f32,
}

impl<'a> InfoPanel<'a> {
    pub fn display(self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            let fid = TextStyle::Body.resolve(ui.style());
            let space = ui.fonts(|f| f.glyph_width(&fid, ' '));
            let height = ui.text_style_height(&TextStyle::Body);

            let mut title = ui
                .horizontal(|ui| {
                    ImageView {
                        texture_id: self.request.image_id.and_then(|fid| self.cache.get(fid)),
                        size: height,
                    }
                    .display(ui);

                    // XXX this is a normal title by artist (user) view
                    // we probably want a scrolling title here

                    RequestView {
                        request: self.request,
                        fid: &fid,
                        space,
                        active: ui.visuals().strong_text_color(),
                        inactive: ui.visuals().text_color(),
                    }
                    .display(ui);
                })
                .response;

            title |= ui.separator();

            let resp = ui.allocate_rect(
                Rect::from_min_size(
                    ui.cursor().min,
                    vec2(ui.available_width(), self.height - title.rect.height()),
                ),
                Sense::hover(),
            );

            let mut ui = ui.child_ui(resp.rect, Layout::default());
            if self.request.lyrics.lyrics.is_empty() {
                // TODO center this
                ui.label("No lyrics available");
                return;
            }

            ui.push_id(self.request.track.id, |ui| {
                LyricsPanel {
                    request: self.request,
                    elapsed: self.elapsed,
                    player: self.player,
                }
                .display(ui);
            });
        });
    }
}
