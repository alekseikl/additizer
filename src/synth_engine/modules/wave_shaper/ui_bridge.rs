use crate::synth_engine::{Input, StereoSample, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{ShaperType, WaveShaper, WaveShaperConfig};

pub struct WaveShaperUiBridge {
    ui_end: UiEnd,
    config: WaveShaperConfig,
}

impl WaveShaperUiBridge {
    pub fn try_new(shaper: &mut WaveShaper) -> Option<Self> {
        Some(Self {
            ui_end: shaper.ui_end.take()?,
            config: shaper.get_config(),
        })
    }

    pub fn config(&self) -> &WaveShaperConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::Distortion => self.config.distortion = value,
            Input::ClippingLevel => self.config.clipping_level = value,
            _ => (),
        }
    }

    pub fn set_shaper_type(&mut self, shaper_type: ShaperType) {
        if self.ui_end.set_shaper_type(shaper_type) {
            self.config.shaper_type = shaper_type;
        }
    }
}

impl ModuleUiBridge for WaveShaperUiBridge {
    fn update(&mut self) {}
}
