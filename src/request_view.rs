use egui::{text::LayoutJob, Color32, CursorIcon, FontId, Label, Sense, TextFormat};

use crate::request::Request;

pub struct RequestView<'a> {
    pub request: &'a Request,
    pub fid: &'a FontId,
    pub space: f32,
    pub active: Color32,
    pub inactive: Color32,
}

impl<'a> RequestView<'a> {
    pub fn display(self, ui: &mut egui::Ui) -> egui::Response {
        let mut job = LayoutJob::simple(
            self.request.track.name.clone(),
            self.fid.clone(),
            self.active,
            0.0,
        );
        job.append(
            " by ",
            self.space,
            TextFormat::simple(self.fid.clone(), self.inactive),
        );

        for (i, artist) in self.request.track.artists.iter().enumerate() {
            if i > 0 {
                job.append(
                    ", ",
                    0.0,
                    TextFormat::simple(self.fid.clone(), self.inactive),
                );
            }
            job.append(
                &artist.name,
                self.space,
                TextFormat::simple(self.fid.clone(), self.active),
            )
        }

        job.append(
            "(",
            self.space,
            TextFormat::simple(self.fid.clone(), self.inactive),
        );

        job.append(
            &format!("{}", self.request.user.name),
            0.0,
            TextFormat::simple(self.fid.clone(), self.request.user.color),
        );

        job.append(
            ")",
            0.0,
            TextFormat::simple(self.fid.clone(), self.inactive),
        );

        let resp = ui
            .add(Label::new(job).wrap(true))
            .interact(Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(self.request.user.id.as_str());

        if resp.clicked_by(egui::PointerButton::Primary) {
            ui.output_mut(|o| {
                o.open_url(format!(
                    "https://open.spotify.com/track/{id}",
                    id = self.request.track.id.to_base62().unwrap()
                ))
            })
        }

        resp
    }
}
