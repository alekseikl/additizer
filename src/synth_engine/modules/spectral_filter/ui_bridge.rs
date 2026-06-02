use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, ModuleId, StereoSample, SynthEngine};

use super::{SpectralFilter, SpectralFilterType};
use super::link::UiEnd;

#[derive(Clone)]
pub struct ControlsState {
    pub filter_type: SpectralFilterType,
    pub cutoff: StereoSample,
    pub q: StereoSample,
    pub drive: StereoSample,
    pub fourth_order: bool,
    pub linear_phase: bool,
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
        let filter = synth_lock.get_typed_module_mut::<SpectralFilter>(module_id)?;
        let ui_end = filter.take_ui_end()?;
        let controls = filter.get_controls_state();

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

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.as_mut().unwrap().set_param(input, value) {
            return;
        }

        match input {
            Input::Cutoff => self.controls.cutoff = value,
            Input::Q => self.controls.q = value,
            Input::Drive => self.controls.drive = value,
            _ => (),
        }
    }

    pub fn set_filter_type(&mut self, filter_type: SpectralFilterType) {
        if self.ui_end.as_mut().unwrap().set_filter_type(filter_type) {
            self.controls.filter_type = filter_type;
        }
    }

    pub fn set_fourth_order(&mut self, value: bool) {
        if self.ui_end.as_mut().unwrap().set_fourth_order(value) {
            self.controls.fourth_order = value;
        }
    }

    pub fn set_linear_phase(&mut self, value: bool) {
        if self.ui_end.as_mut().unwrap().set_linear_phase(value) {
            self.controls.linear_phase = value;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(filter) = synth_lock.get_typed_module_mut::<SpectralFilter>(self.module_id) {
            filter.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
