use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{ModuleId, Sample, SynthEngine};

use super::link::UiEnd;
use super::{ExternalParam, ExternalParamConfig, NUM_FLOAT_PARAMS};

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    config: ExternalParamConfig,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let param = synth_lock.get_typed_module_mut::<ExternalParam>(module_id)?;
        let ui_end = param.take_ui_end()?;
        let config = param.get_config();

        drop(synth_lock);

        Some(Self {
            synth,
            module_id,
            ui_end: Some(ui_end),
            config,
        })
    }

    pub fn module_id(&self) -> ModuleId {
        self.module_id
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

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(param) = synth_lock.get_typed_module_mut::<ExternalParam>(self.module_id) {
            param.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
