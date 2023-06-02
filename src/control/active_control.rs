use std::{collections::VecDeque, time::Duration};

use egui::{vec2, Color32, CursorIcon, Layout, Rect, Rounding, Sense, TextStyle};

use librespot::playback::player::Player;

use crate::{
    image_cache::ImageCache, player_state::PlayerState, progress::Progress, request::Request,
    util::format_duration, views::ImageView, volume_state::VolumeState,
};

use super::player_control::PlayerControl;

pub struct ActiveControl<'a> {
    pub cache: &'a mut ImageCache,
    pub queue: &'a mut VecDeque<Request>,

    pub elapsed: Option<usize>,

    pub has_active: bool,
    pub auto_play: &'a mut bool,

    pub player: &'a mut Player,
    pub player_state: &'a PlayerState,
    pub volume: &'a VolumeState,

    pub request: &'a mut Request,
    pub play_pos: &'a mut Option<Duration>,
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

            ui.painter().rect_filled(
                resp.rect, //
                Rounding::none(),
                Color32::from_black_alpha(64),
            );

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

            if self.player_state.is_playing()
                && !self.player_state.is_loading(&self.request.track.id)
            {
                let delta = Duration::from_secs_f32(ui.input(|i| i.unstable_dt));
                *self.play_pos.get_or_insert_with(Duration::default) += delta;
            }

            if let Some(elapsed) = self.elapsed {
                let text_height = ui.text_style_height(&TextStyle::Monospace);
                let resp = ui.allocate_rect(
                    Rect::from_min_size(
                        resp.rect.left_bottom() + vec2(0.0, 4.0), //
                        vec2(resp.rect.width(), text_height),
                    ),
                    Sense::hover(),
                );

                let mut progress_ui = ui.child_ui(resp.rect, Layout::default());

                let dur = self.request.track.duration as f32;
                let diff = dur - elapsed as f32;
                let pos = 1.0 - egui::emath::inverse_lerp(0.0..=dur, diff).unwrap();

                let mut seek_to = None;
                let resp = Progress::new(pos, self.request.track.duration as _)
                    .with_bg_color(ui.visuals().extreme_bg_color)
                    .with_fill_color(Color32::from_rgb(0x1F, 0xDF, 0x64))
                    .with_galley(ui.painter().layout_no_wrap(
                        format!(
                            "{} / {}",
                            format_duration(elapsed as _),
                            format_duration(self.request.track.duration as _)
                        ),
                        TextStyle::Monospace.resolve(ui.style()),
                        Color32::TRANSPARENT,
                    ))
                    .display(&mut progress_ui, &mut seek_to);

                if resp.drag_released() {
                    if let Some(offset) = seek_to {
                        log::debug!("seek to: {offset}ms");
                        self.player.seek(offset);
                    }
                }
            }

            PlayerControl {
                player_state: self.player_state,
                player: self.player,
                request: self.request,
                queue: self.queue,
                auto_play: self.auto_play,
                has_active: self.has_active,
                volume: self.volume,
            }
            .display(ui, replace);
        })
        .response
    }
}
