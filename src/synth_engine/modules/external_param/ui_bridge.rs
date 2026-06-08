use crate::synth_engine::{Sample, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{ExternalParam, ExternalParamConfig, NUM_FLOAT_PARAMS};

pub struct ExternalParamUiBridge {
    ui_end: UiEnd,
    config: ExternalParamConfig,
}

impl ExternalParamUiBridge {
    pub fn try_new(param: &mut ExternalParam) -> Option<Self> {
        Some(Self {
            ui_end: param.ui_end.take()?,
            config: param.get_config(),
        })
    }

    pub fn config(&self) -> &ExternalParamConfig {
        &self.config
    }

    pub fn select_param(&mut self, index: usize) {
        if self.ui_end.select_param(index) {
            self.config.selected_param_index = index.min(NUM_FLOAT_PARAMS - 1);
        }
    }

    pub fn set_smooth(&mut self, value: Sample) {
        if self.ui_end.set_smooth(value) {
            self.config.smooth = value;
        }
    }

    pub fn set_sample_and_hold(&mut self, value: bool) {
        if self.ui_end.set_sample_and_hold(value) {
            self.config.sample_and_hold = value;
        }
    }
}

impl ModuleUiBridge for ExternalParamUiBridge {
    fn update(&mut self) {}
}
