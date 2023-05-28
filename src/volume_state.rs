use std::sync::Arc;

use egui::mutex::Mutex;
use librespot::playback::mixer::VolumeGetter;

#[derive(Clone)]
pub struct VolumeState {
    pub(crate) volume: Arc<Mutex<f64>>,
}

impl VolumeState {
    pub fn get(&self) -> f64 {
        *self.volume.lock()
    }

    pub fn set(&self, volume: f64) {
        *self.volume.lock() = volume
    }
}

impl VolumeGetter for VolumeState {
    fn attenuation_factor(&self) -> f64 {
        self.get()
    }
}
