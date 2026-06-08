use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{
    Input, ModuleId, StereoSample, SynthEngine, synth_module::ModuleUiBridge,
};

use super::link::UiEnd;
use super::{ShaperType, WaveShaper, WaveShaperConfig};

pub struct WaveShaperUiBridge {
    ui_end: Option<UiEnd>,
    config: WaveShaperConfig,
}

impl WaveShaperUiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let shaper = synth_lock.get_typed_module_mut::<WaveShaper>(module_id)?;
        let ui_end = shaper.ui_end.take()?;
        let config = shaper.get_config();

        drop(synth_lock);

        Some(Self {
            ui_end: Some(ui_end),
            config,
        })
    }

    pub fn config(&self) -> &WaveShaperConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.as_mut().unwrap().set_param(input, value) {
            return;
        }

        match input {
            Input::Distortion => self.config.distortion = value,
            Input::ClippingLevel => self.config.clipping_level = value,
            _ => (),
        }
    }

    pub fn set_shaper_type(&mut self, shaper_type: ShaperType) {
        if self.ui_end.as_mut().unwrap().set_shaper_type(shaper_type) {
            self.config.shaper_type = shaper_type;
        }
    }
}

impl ModuleUiBridge for WaveShaperUiBridge {
    fn update(&mut self) {}
}
