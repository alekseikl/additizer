use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{ModuleId, Sample, SynthEngine, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{ExternalParam, ExternalParamConfig, NUM_FLOAT_PARAMS};

pub struct ExternalParamUiBridge {
    ui_end: Option<UiEnd>,
    config: ExternalParamConfig,
}

impl ExternalParamUiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let param = synth_lock.get_typed_module_mut::<ExternalParam>(module_id)?;
        let ui_end = param.ui_end.take()?;
        let config = param.get_config();

        drop(synth_lock);

        Some(Self {
            ui_end: Some(ui_end),
            config,
        })
    }

    pub fn config(&self) -> &ExternalParamConfig {
        &self.config
    }

    pub fn select_param(&mut self, index: usize) {
        if self.ui_end.as_mut().unwrap().select_param(index) {
            self.config.selected_param_index = index.min(NUM_FLOAT_PARAMS - 1);
        }
    }

    pub fn set_smooth(&mut self, value: Sample) {
        if self.ui_end.as_mut().unwrap().set_smooth(value) {
            self.config.smooth = value;
        }
    }

    pub fn set_sample_and_hold(&mut self, value: bool) {
        if self.ui_end.as_mut().unwrap().set_sample_and_hold(value) {
            self.config.sample_and_hold = value;
        }
    }
}

impl ModuleUiBridge for ExternalParamUiBridge {
    fn update(&mut self) {}
}
