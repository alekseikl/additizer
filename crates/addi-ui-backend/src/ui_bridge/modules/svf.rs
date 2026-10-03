use crate::ui_bridge::ModuleUiBridge;
use addi_dsp::filters::svf::SvfType;
use addi_engine::svf::{Svf, SvfConfig, SvfLinks, SvfUiEnd};
use addi_engine::{Input, Sample, StereoSample};

pub struct SvfUiBridge<L: SvfLinks = crate::links::svf::Links> {
    ui_end: L::UiEnd,
    config: SvfConfig,
}

impl<L: SvfLinks> SvfUiBridge<L> {
    pub fn try_new(svf: &mut Svf<L>) -> Option<Self> {
        Some(Self {
            ui_end: svf.take_ui_end()?,
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

impl<L: SvfLinks> ModuleUiBridge for SvfUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
