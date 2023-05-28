use std::sync::Arc;

use egui::{vec2, Galley, Layout, Rect, Sense};

#[derive(Clone)]
pub struct Scrollable {
    pub galley: Arc<Galley>,
    pub pos: f32,
    pub speed: f32,
}

impl Scrollable {
    pub fn new(galley: Arc<Galley>) -> Self {
        Self {
            pos: f32::INFINITY,
            galley,
            speed: 10.0,
        }
    }

    pub fn with_speed(self, speed: f32) -> Self {
        Self { speed, ..self }
    }

    pub fn display(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let mut target = ui
            .available_rect_before_wrap()
            .intersect(Rect::from_min_size(
                ui.available_rect_before_wrap().min,
                self.galley.size(),
            ));
        target.extend_with_x(ui.available_width());

        let resp = ui.allocate_rect(target, Sense::hover());

        if self.galley.size().x <= target.width() {
            ui.painter().galley(
                target.left_top(), //
                self.galley.clone(),
            );
            self.pos = f32::INFINITY;
            return resp;
        }

        let mut ui = ui.child_ui(target, Layout::default());
        ui.set_clip_rect(target);

        if self.pos == f32::INFINITY {
            self.pos = target.width().mul_add(-0.25, target.right());
        }

        self.pos -= ui.input(|i| i.stable_dt.min(0.1)) * self.speed;
        let rect = target.translate(vec2(self.pos, 0.0));

        ui.painter().galley(
            rect.left_top(), //
            self.galley.clone(),
        );

        if self.pos.abs() >= self.galley.size().x {
            self.pos = target.right();
        }

        ui.ctx().request_repaint();
        resp
    }
}
