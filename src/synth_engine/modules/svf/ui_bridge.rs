use crate::synth_engine::{
    Input, Sample, StereoSample, filters::svf::SvfType, synth_module::ModuleUiBridge,
};

use super::link::UiEnd;
use super::{Svf, SvfConfig};

pub struct SvfUiBridge {
    ui_end: UiEnd,
    config: SvfConfig,
}

impl SvfUiBridge {
    pub fn try_new(svf: &mut Svf) -> Option<Self> {
        Some(Self {
            ui_end: svf.ui_end.take()?,
            config: svf.get_config(),
        })
    }

    pub fn config(&self) -> &SvfConfig {
        &self.config
    }

    /// Pitch of the most recent voice, for keytrack display.
    pub fn pitch(&mut self) -> Sample {
        self.ui_end.pitch()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::Cutoff => self.config.cutoff = value,
            Input::Resonance => self.config.resonance = value,
            Input::Drive => self.config.drive = value,
            _ => (),
        }
    }

    pub fn set_filter_type(&mut self, filter_type: SvfType) {
        if self.ui_end.set_filter_type(filter_type) {
            self.config.filter_type = filter_type;
        }
    }

    pub fn set_keytrack(&mut self, value: Sample) {
        if self.ui_end.set_keytrack(value) {
            self.config.keytrack = value;
        }
    }
}

impl ModuleUiBridge for SvfUiBridge {
    fn update(&mut self) -> bool {
        false
    }
}
