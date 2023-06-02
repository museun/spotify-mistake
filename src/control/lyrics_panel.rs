use egui::{Label, RichText, ScrollArea, Sense};
use librespot::playback::player::Player;

use crate::request::Request;

pub struct LyricsPanel<'a> {
    pub request: &'a Request,
    pub elapsed: Option<usize>,
    pub player: &'a Player,
}

impl<'a> LyricsPanel<'a> {
    pub fn display(self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            let len = self.request.lyrics.lyrics.len();
            for (i, line) in self.request.lyrics.lyrics.iter().enumerate() {
                let mut should_scroll = false;
                let end = if i < len {
                    line.end
                } else {
                    self.request.track.duration as _
                };
                let range = line.start..=end;

                let color = self
                    .elapsed
                    .filter(|_| self.request.lyrics.synced)
                    .map_or_else(
                        || ui.visuals().text_color(),
                        |elapsed| {
                            range
                                .contains(&(elapsed as _))
                                .then(|| {
                                    should_scroll = true;
                                    ui.visuals().warn_fg_color
                                })
                                .unwrap_or_else(|| ui.visuals().text_color())
                        },
                    );

                let resp = ui
                    .add(Label::new(RichText::new(&line.data).color(color)).wrap(true))
                    .interact(Sense::click());

                if resp.clicked() && self.request.lyrics.synced {
                    log::debug!("seek to: {}ms", line.start);
                    self.player.seek(line.start as _);
                }

                if should_scroll {
                    resp.scroll_to_me(Some(egui::Align::Center));
                }
            }

            ui.allocate_space(ui.available_size_before_wrap());
        });
    }
}
