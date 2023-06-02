use std::sync::Arc;

use egui::{
    pos2, vec2, Align2, Color32, Galley, LayerId, Order, Rect, Rounding, Sense, Stroke, TextStyle,
};

use crate::util::format_duration;
pub struct Progress {
    pos: f32,
    total: u32,
    bg_color: Color32,
    fill_color: Color32,
    galley: Option<Arc<Galley>>,
}

impl Progress {
    pub fn new(pos: f32, total: u32) -> Self {
        Self {
            pos,
            total,
            bg_color: Color32::BLACK,
            fill_color: Color32::GREEN,
            galley: None,
        }
    }

    pub fn with_bg_color(self, bg_color: Color32) -> Self {
        Self { bg_color, ..self }
    }

    pub fn with_fill_color(self, fill_color: Color32) -> Self {
        Self { fill_color, ..self }
    }

    pub fn with_galley(self, galley: Arc<Galley>) -> Self {
        Self {
            galley: Some(galley),
            ..self
        }
    }

    pub fn display(self, ui: &mut egui::Ui, seeked: &mut Option<u32>) -> egui::Response {
        let w = ui.available_size_before_wrap().x;
        let h = (ui.spacing().interact_size.y * 0.6).max(ui.available_height());

        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
        if !ui.is_rect_visible(rect) {
            return resp;
        }

        ui.painter().rect(
            rect,
            Rounding::none(), //
            self.bg_color,
            Stroke::NONE,
        );

        let diff = self.pos / 1.0;

        let fill_rect = Rect::from_min_size(rect.min, vec2(rect.width() * diff, rect.height()));
        ui.painter().rect(
            fill_rect, //
            Rounding::none(),
            self.fill_color,
            Stroke::NONE,
        );

        let drag_id = egui::Id::new("progress").with("delta");

        if let Some(offset) = ui.data(|d| d.get_temp::<u32>(drag_id)) {
            if let Some(pos) = ui.ctx().pointer_latest_pos() {
                let offset_text = format_duration(offset);
                let fid = TextStyle::Monospace.resolve(ui.style());
                let galley = ui.fonts(|f| f.layout_delayed_color(offset_text, fid, f32::INFINITY));

                let pos = pos.max(rect.min).min(rect.max);

                let mut text_rect = Align2::CENTER_CENTER
                    .align_size_within_rect(
                        galley.size(),
                        Rect::from_min_size(pos2(pos.x, rect.top() - rect.height()), galley.size()),
                    )
                    .translate(vec2(0.0, -2.0));
                if text_rect.right() > rect.right() {
                    text_rect = text_rect.translate(-vec2(text_rect.right() - rect.right(), 0.0));
                }

                let p = ui.ctx().layer_painter(LayerId::new(
                    Order::Foreground,
                    egui::Id::new("progress-info-text"),
                ));
                p.rect_filled(text_rect, Rounding::none(), ui.visuals().extreme_bg_color);

                p.galley_with_color(
                    text_rect.left_top(),
                    galley,
                    ui.visuals().strong_text_color(),
                );
                p.line_segment(
                    [text_rect.left_bottom(), text_rect.right_bottom()],
                    (1.0, ui.visuals().warn_fg_color),
                );

                p.line_segment(
                    [
                        pos2(pos.x + 0.25, text_rect.bottom()),
                        pos2(pos.x + 0.25, rect.bottom()),
                    ],
                    (2.0, ui.visuals().warn_fg_color),
                );
            }
        }

        if let Some(galley) = self.galley {
            let text_rect = Align2::CENTER_CENTER.align_size_within_rect(galley.size(), rect);

            let text_mask = text_rect.expand2(vec2(4.0, 0.0));
            let mask_color = if fill_rect.intersects(text_mask) {
                self.fill_color.gamma_multiply(0.4).to_opaque()
            } else {
                Color32::TRANSPARENT
            };

            ui.painter().rect(
                fill_rect.intersect(text_mask),
                Rounding::none(),
                mask_color,
                Stroke::NONE,
            );

            ui.painter().galley_with_color(
                text_rect.left_top(),
                galley,
                ui.visuals().strong_text_color(),
            );
        }

        if resp.dragged_by(egui::PointerButton::Primary) && ui.input(|i| i.modifiers.shift_only()) {
            ui.input(|i| i.pointer.interact_pos()).map_or_else(
                || seeked.take().map(|_| ()).unwrap_or_default(),
                |pos| {
                    let range = (egui::emath::remap_clamp(
                        pos.x, //
                        rect.x_range(),
                        0.0..=1.0,
                    ) * self.total as f32) as _;

                    ui.data_mut(|d| {
                        d.insert_temp::<u32>(drag_id, range);
                    })
                },
            );
        }

        if resp.drag_released() {
            ui.data_mut(|d| {
                let val = d.get_temp::<u32>(drag_id);
                if let Some(val) = val {
                    seeked.replace(val);
                    d.remove::<u32>(drag_id)
                }
            });
        }
        resp
    }
}
