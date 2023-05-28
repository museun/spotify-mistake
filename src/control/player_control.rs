use std::collections::VecDeque;

use egui::Slider;
use librespot::playback::player::Player;

use crate::{player_state::PlayerState, request::Request, volume_state::VolumeState};

pub struct PlayerControl<'a> {
    pub player_state: &'a PlayerState,
    pub player: &'a mut Player,
    pub request: &'a Request,
    pub queue: &'a mut VecDeque<Request>,
    pub auto_play: &'a mut bool,
    pub volume: &'a VolumeState,
}

impl<'a> PlayerControl<'a> {
    pub fn display(self, ui: &mut egui::Ui, replace: &mut Option<Request>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                if self.player_state.is_paused() {
                    if ui.small_button("Resume").clicked() {
                        self.player.play();
                    }
                } else if self.player_state.is_not_playing() || self.player_state.is_done_playing()
                {
                    if ui.small_button("Play").clicked() {
                        self.player.load(self.request.track.id, true, 0);
                    }
                } else if self.player_state.is_loading() {
                    ui.spinner();
                } else if self.player_state.is_playing() && ui.small_button("Pause").clicked() {
                    self.player.pause();
                }

                if ui.small_button("Skip").clicked() {
                    *replace = self.queue.pop_front();
                }
                ui.toggle_value(self.auto_play, "Auto");
            });

            ui.scope(|ui| {
                let mut vol = self.volume.volume.lock();
                ui.spacing_mut().slider_width = 128.0;
                ui.add(
                    Slider::new(&mut *vol, 0.0..=1.0)
                        .step_by(0.01)
                        .trailing_fill(true)
                        .show_value(false),
                )
                .on_hover_text(format!("Volume factor: {vol:.2?}", vol = *vol));
            });
        });
    }
}
