use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, ModuleId, StereoSample, SynthEngine};

use super::{Lfo, LfoShape};
use super::link::UiEnd;

#[derive(Clone)]
pub struct ControlsState {
    pub shape: LfoShape,
    pub bipolar: bool,
    pub steal_phase: bool,
    pub frequency: StereoSample,
    pub phase_shift: StereoSample,
    pub skew: StereoSample,
    pub smooth_time: StereoSample,
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
        let lfo = synth_lock.get_typed_module_mut::<Lfo>(module_id)?;
        let ui_end = lfo.take_ui_end()?;
        let controls = lfo.get_controls_state();

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
            Input::LowFrequency => self.controls.frequency = value,
            Input::PhaseShift => self.controls.phase_shift = value,
            Input::Skew => self.controls.skew = value,
            _ => (),
        }
    }

    pub fn set_shape(&mut self, shape: LfoShape) {
        if self.ui_end.as_mut().unwrap().set_shape(shape) {
            self.controls.shape = shape;
        }
    }

    pub fn set_bipolar(&mut self, value: bool) {
        if self.ui_end.as_mut().unwrap().set_bipolar(value) {
            self.controls.bipolar = value;
        }
    }

    pub fn set_steal_phase(&mut self, value: bool) {
        if self.ui_end.as_mut().unwrap().set_steal_phase(value) {
            self.controls.steal_phase = value;
        }
    }

    pub fn set_smooth_time(&mut self, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_smooth_time(value) {
            self.controls.smooth_time = value;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(lfo) = synth_lock.get_typed_module_mut::<Lfo>(self.module_id) {
            lfo.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
