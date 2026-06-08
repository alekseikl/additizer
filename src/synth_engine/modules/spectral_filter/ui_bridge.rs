use crate::synth_engine::{Input, StereoSample, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{SpectralFilter, SpectralFilterConfig, SpectralFilterType};

pub struct SpectralFilterUiBridge {
    ui_end: UiEnd,
    config: SpectralFilterConfig,
}

impl SpectralFilterUiBridge {
    pub fn try_new(filter: &mut SpectralFilter) -> Option<Self> {
        Some(Self {
            ui_end: filter.ui_end.take()?,
            config: filter.get_config(),
        })
    }

    pub fn config(&self) -> &SpectralFilterConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::Cutoff => self.config.cutoff = value,
            Input::Q => self.config.q = value,
            Input::Drive => self.config.drive = value,
            _ => (),
        }
    }

    pub fn set_filter_type(&mut self, filter_type: SpectralFilterType) {
        if self.ui_end.set_filter_type(filter_type) {
            self.config.filter_type = filter_type;
        }
    }

    pub fn set_fourth_order(&mut self, value: bool) {
        if self.ui_end.set_fourth_order(value) {
            self.config.fourth_order = value;
        }
    }

    pub fn set_linear_phase(&mut self, value: bool) {
        if self.ui_end.set_linear_phase(value) {
            self.config.linear_phase = value;
        }
    }
}

impl ModuleUiBridge for SpectralFilterUiBridge {
    fn update(&mut self) {}
}
