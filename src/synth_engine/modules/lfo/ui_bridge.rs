use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, ModuleId, StereoSample, SynthEngine};

use super::link::UiEnd;
use super::{Lfo, LfoConfig, LfoShape};

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    config: LfoConfig,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let lfo = synth_lock.get_typed_module_mut::<Lfo>(module_id)?;
        let ui_end = lfo.take_ui_end()?;
        let config = lfo.get_config();

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

    pub fn config(&self) -> &LfoConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.as_mut().unwrap().set_param(input, value) {
            return;
        }

        match input {
            Input::LowFrequency => self.config.frequency = value,
            Input::PhaseShift => self.config.phase_shift = value,
            Input::Skew => self.config.skew = value,
            _ => (),
        }
    }

    pub fn set_shape(&mut self, shape: LfoShape) {
        if self.ui_end.as_mut().unwrap().set_shape(shape) {
            self.config.shape = shape;
        }
    }

    pub fn set_bipolar(&mut self, value: bool) {
        if self.ui_end.as_mut().unwrap().set_bipolar(value) {
            self.config.bipolar = value;
        }
    }

    pub fn set_steal_phase(&mut self, value: bool) {
        if self.ui_end.as_mut().unwrap().set_steal_phase(value) {
            self.config.steal_phase = value;
        }
    }

    pub fn set_smooth_time(&mut self, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_smooth_time(value) {
            self.config.smooth_time = value;
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
