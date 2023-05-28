use egui::{Color32, ScrollArea, TextStyle};

use crate::{
    image_cache,
    util::format_duration,
    views::{ImageView, RequestView},
    Request,
};

pub struct ListView<'a> {
    pub cache: &'a mut image_cache::ImageCache,
}

impl<'a> ListView<'a> {
    pub fn display<'i>(
        self,
        ui: &mut egui::Ui,
        empty_label: &'static str,
        is_empty: bool,
        buttons: impl Fn(&mut egui::Ui, &mut Option<Request>, &Request),
        items: impl Iterator<Item = &'i Request>,
    ) -> Action {
        let mut remove = None;
        let mut add = None;

        ScrollArea::vertical().show(ui, |ui| {
            if is_empty {
                ui.label(empty_label);
                return;
            }

            let fid = TextStyle::Body.resolve(ui.style());
            let space = ui.fonts(|f| f.glyph_width(&fid, ' '));
            let height = ui.text_style_height(&TextStyle::Body);

            // TODO allow searching by user (e.g. click on user name and show all songs from them)
            // TODO allow reordering
            // TODO use a table here
            for (i, next) in items.enumerate() {
                ui.horizontal(|ui| {
                    // TODO make this a context menu
                    buttons(ui, &mut add, next);

                    // TODO make this a context menu
                    if ui.small_button("🚫").clicked() {
                        remove.replace(i);
                    }

                    ui.monospace(format_duration(next.track.duration as _));

                    ImageView {
                        texture_id: next.image_id.and_then(|fid| self.cache.get(fid)),
                        size: height,
                    }
                    .display(ui);

                    RequestView {
                        request: next,
                        fid: &fid,
                        space,
                        active: Color32::WHITE,
                        inactive: ui.visuals().text_color(),
                    }
                    .display(ui);

                    ui.allocate_space(ui.available_size_before_wrap());
                });
            }

            ui.allocate_space(ui.available_size_before_wrap());
        });

        remove
            .map(|index| Action::Remove { index })
            .or_else(|| add.map(|request| Action::Add { request }))
            .unwrap_or_default()
    }
}

#[derive(Default)]
pub enum Action {
    Add {
        request: Request,
    },
    Remove {
        index: usize,
    },
    #[default]
    Nothing,
}
