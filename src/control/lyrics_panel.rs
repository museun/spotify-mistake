use egui::{Align, Color32, Label, RichText, ScrollArea};

use crate::request::Request;

pub struct LyricsPanel<'a> {
    pub request: &'a Request,
    pub elapsed: Option<usize>,
}

impl<'a> LyricsPanel<'a> {
    pub fn display(self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            for line in &*self.request.lyrics.lyrics {
                let mut should_scroll = false;

                let color = self
                    .elapsed
                    .filter(|_| self.request.lyrics.synced)
                    .map_or_else(
                        || ui.visuals().text_color(),
                        |elapsed| {
                            (line.start..=line.end)
                                .contains(&(elapsed as _))
                                .then(|| {
                                    should_scroll = true;
                                    Color32::WHITE
                                })
                                .unwrap_or_else(|| ui.visuals().text_color())
                        },
                    );

                let resp = ui.add(Label::new(RichText::new(&line.data).color(color)).wrap(true));

                if should_scroll {
                    resp.scroll_to_me(Some(Align::Center))
                }
            }
            ui.allocate_space(ui.available_size_before_wrap());
        });
    }
}
