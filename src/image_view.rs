use egui::{TextureId, Vec2};

pub struct ImageView {
    pub texture_id: Option<TextureId>,
    pub size: f32,
}
impl ImageView {
    pub fn display(self, ui: &mut egui::Ui) -> egui::Response {
        match self.texture_id {
            Some(id) => ui.add(egui::Image::new(id, Vec2::splat(self.size))),
            None => ui.spinner(),
        }
    }
}
