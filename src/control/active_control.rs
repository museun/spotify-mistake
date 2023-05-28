use std::{collections::VecDeque, time::Instant};

use egui::{
    text::LayoutJob, vec2, Align, Align2, Color32, CursorIcon, Layout, Rounding, Sense, TextFormat,
    TextStyle,
};

use librespot::playback::player::Player;

use crate::{
    ext::JoinWith, image_cache::ImageCache, player_state::PlayerState, progress::Progress,
    request::Request, scrollable::Scrollable, util::format_duration, views::ImageView,
    volume_state::VolumeState,
};

use super::player_control::PlayerControl;

pub struct ActiveControl<'a> {
    pub cache: &'a mut ImageCache,
    pub queue: &'a mut VecDeque<Request>,

    pub elapsed: Option<usize>,

    pub auto_play: &'a mut bool,

    pub player: &'a mut Player,
    pub player_state: &'a PlayerState,
    pub volume: &'a VolumeState,

    pub request: &'a mut Request,
    pub start: &'a mut Option<Instant>,
}

impl<'a> ActiveControl<'a> {
    pub fn display(self, ui: &mut egui::Ui, replace: &mut Option<Request>) -> egui::Response {
        ui.vertical(|ui| {
            let Some(image_id) = self.request.image_id else {
                ui.heading("No active song");
                return
            };

            let resp = ImageView {
                texture_id: self.cache.get(image_id),
                size: 128.0,
            }
            .display(ui);

            ui.painter()
                .rect_filled(resp.rect, Rounding::none(), Color32::from_black_alpha(64));

            let resp = resp
                .interact(Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);

            if resp.clicked_by(egui::PointerButton::Primary) {
                ui.output_mut(|o| {
                    o.open_url(format!(
                        "https://open.spotify.com/track/{id}",
                        id = self.request.track.id.to_base62().unwrap()
                    ))
                })
            }

            if self.player_state.is_playing() && !self.player_state.is_loading() {
                self.start.get_or_insert_with(Instant::now);
            }

            if let Some(elapsed) = self.elapsed {
                let fid = TextStyle::Monospace.resolve(ui.style());
                let galley = ui.fonts(|f| {
                    f.layout(
                        format!(
                            "{} / {}",
                            format_duration(elapsed as _),
                            format_duration(self.request.track.duration as _)
                        ),
                        fid,
                        Color32::WHITE,
                        f32::INFINITY,
                    )
                });

                let duration_rect = Align2::RIGHT_TOP
                    .align_size_within_rect(galley.size(), resp.rect)
                    .expand2(vec2(4.0, 1.0))
                    .intersect(resp.rect);

                ui.painter().rect_filled(
                    duration_rect,
                    Rounding::none(),
                    Color32::from_black_alpha(0x90),
                );

                ui.painter()
                    .galley(duration_rect.left_top() + vec2(2.0, 0.0), galley);
            }

            let fid = TextStyle::Heading.resolve(ui.style());

            let id = egui::Id::new(self.request.track.id);
            let scrollable = ui.data_mut(|d| d.get_temp::<Scrollable>(id));

            let mut scrollable = scrollable.unwrap_or_else(|| {
                let galley = ui.fonts(|f| {
                    f.layout_job({
                        let mut job = LayoutJob::simple_singleline(
                            self.request.track.name.clone(),
                            fid.clone(),
                            Color32::WHITE,
                        );
                        job.append(
                            " by ",
                            4.0,
                            TextFormat::simple(fid.clone(), ui.visuals().text_color()),
                        );
                        let artists = &self.request.track.artists;
                        job.append(
                            &artists.iter().map(|c| &c.name).join(", "),
                            4.0,
                            TextFormat::simple(fid, Color32::WHITE),
                        );
                        job
                    })
                });
                Scrollable::new(galley).with_speed(75.0)
            });

            let galley_rect = Align2::CENTER_CENTER
                .align_size_within_rect(scrollable.galley.size(), resp.rect)
                .intersect(resp.rect);

            ui.painter().rect_filled(
                galley_rect,
                Rounding::none(),
                Color32::from_black_alpha(0x90),
            );

            scrollable.display(&mut ui.child_ui(galley_rect, Layout::default()));

            ui.data_mut(|d| d.insert_temp(id, scrollable));

            if let Some(elapsed) = self.elapsed {
                let mut progress_ui = ui.child_ui(
                    resp.rect.shrink2(vec2(2.0, 1.0)),
                    Layout::bottom_up(Align::Min),
                );

                let dur = self.request.track.duration as f32;
                let diff = dur - elapsed as f32;
                let pos = 1.0 - egui::emath::inverse_lerp(0.0..=dur, diff).unwrap();
                if pos >= 1.0 {
                    *replace = self.queue.pop_front();
                    self.start.take();
                }

                let resp = Progress {
                    pos,
                    bg_color: Color32::BLACK,
                    fill_color: Color32::from_rgb(0x1F, 0xDF, 0x64),
                    text: None,
                }
                .display(&mut progress_ui);

                ui.painter()
                    .rect_stroke(resp.rect, Rounding::none(), (0.5, Color32::BLACK));
            }

            PlayerControl {
                player_state: self.player_state,
                player: self.player,
                request: self.request,
                queue: self.queue,
                auto_play: self.auto_play,
                volume: self.volume,
            }
            .display(ui, replace);
        })
        .response
    }
}
