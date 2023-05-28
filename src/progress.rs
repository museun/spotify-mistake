use egui::{pos2, vec2, Align2, Color32, Rect, Rounding, Sense, Stroke, TextStyle};

pub struct Progress<'a> {
    pub pos: f32,
    pub bg_color: Color32,
    pub fill_color: Color32,
    pub text: Option<&'a str>,
}

impl<'a> Progress<'a> {
    pub fn display(self, ui: &mut egui::Ui) -> egui::Response {
        let fid = TextStyle::Monospace.resolve(ui.style());
        let row_height = ui.fonts(|f| f.row_height(&fid));

        let w = ui.available_size_before_wrap().x;
        let h = if self.text.is_some() {
            (ui.spacing().interact_size.y * 0.6).max(row_height)
        } else {
            // TODO just use the available height. the caller can restrict the ui
            8.0
        };

        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
        if !ui.is_rect_visible(rect) {
            return resp;
        }

        ui.painter()
            .rect(rect, Rounding::none(), self.bg_color, Stroke::NONE);

        let diff = self.pos / 1.0;

        let fill_rect = Rect::from_min_size(rect.min, vec2(rect.width() * diff, rect.height()));
        ui.painter()
            .rect(fill_rect, Rounding::none(), self.fill_color, Stroke::NONE);

        if let Some(text) = self.text {
            let text_width =
                ui.fonts(|f| text.chars().fold(0.0, |a, c| a + f.glyph_width(&fid, c)));

            let text_rect = Rect::from_min_size(rect.left_top(), vec2(text_width, row_height))
                .expand2(vec2(2.0, -1.0))
                .translate(vec2(2.0, 0.0));

            ui.painter().rect(
                text_rect,
                Rounding::same(2.0),
                Color32::BLACK.gamma_multiply(0.5),
                Stroke::NONE,
            );

            ui.painter().text(
                pos2(
                    rect.left_top().x + 2.0,
                    row_height.mul_add(0.5, rect.left_top().y),
                ),
                Align2::LEFT_CENTER,
                text,
                fid,
                Color32::WHITE,
            );
        }

        resp
    }
}
