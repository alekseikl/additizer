use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{ModuleId, Sample, SynthEngine};

use super::{ExternalParam, NUM_FLOAT_PARAMS};
use super::link::UiEnd;

#[derive(Clone)]
pub struct ControlsState {
    pub selected_param_index: usize,
    pub num_of_params: usize,
    pub smooth: Sample,
    pub sample_and_hold: bool,
}

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    controls: ControlsState,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let param = synth_lock.get_typed_module_mut::<ExternalParam>(module_id)?;
        let ui_end = param.take_ui_end()?;
        let controls = param.get_controls_state();

        drop(synth_lock);

        Some(Self {
            synth,
            module_id,
            ui_end: Some(ui_end),
            controls,
        })
    }

    pub fn module_id(&self) -> ModuleId {
        self.module_id
    }

    pub fn controls(&self) -> &ControlsState {
        &self.controls
    }

    pub fn select_param(&mut self, index: usize) {
        if self.ui_end.as_mut().unwrap().select_param(index) {
            self.controls.selected_param_index = index.min(NUM_FLOAT_PARAMS - 1);
        }
    }

    pub fn set_smooth(&mut self, value: Sample) {
        if self.ui_end.as_mut().unwrap().set_smooth(value) {
            self.controls.smooth = value;
        }
    }

    pub fn set_sample_and_hold(&mut self, value: bool) {
        if self.ui_end.as_mut().unwrap().set_sample_and_hold(value) {
            self.controls.sample_and_hold = value;
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
