use crate::synth_engine::{Input, StereoSample, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{Amplifier, AmplifierConfig};

pub struct AmplifierUiBridge {
    ui_end: UiEnd,
    config: AmplifierConfig,
}

impl AmplifierUiBridge {
    pub fn try_new(amp: &mut Amplifier) -> Option<Self> {
        Some(Self {
            ui_end: amp.ui_end.take()?,
            config: amp.get_config(),
        })
    }

    pub fn config(&self) -> &AmplifierConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.set_param(input, value) && input == Input::Gain {
            self.config.gain = value;
        }
    }
}

impl ModuleUiBridge for AmplifierUiBridge {
    fn update(&mut self) {}
}
