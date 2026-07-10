use crate::synth_engine::{Input, StereoSample, synth_module::ModuleUiBridge};

use super::link::{DisplaySpectrum, UiEnd};
use super::{SpectralBlend, SpectralBlendConfig};

pub struct SpectralBlendUiBridge {
    ui_end: UiEnd,
    config: SpectralBlendConfig,
}

impl SpectralBlendUiBridge {
    pub fn try_new(blend: &mut SpectralBlend) -> Option<Self> {
        Some(Self {
            ui_end: blend.ui_end.take()?,
            config: blend.get_config(),
        })
    }

    pub fn config(&self) -> &SpectralBlendConfig {
        &self.config
    }

    pub fn get_spectrum(&mut self) -> &DisplaySpectrum {
        self.ui_end.get_spectrum()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.set_param(input, value) && input == Input::Blend {
            self.config.blend = value;
        }
    }
}

impl ModuleUiBridge for SpectralBlendUiBridge {
    fn update(&mut self) -> bool {
        false
    }
}
