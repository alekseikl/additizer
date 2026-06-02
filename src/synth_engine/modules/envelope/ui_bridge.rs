use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, ModuleId, StereoSample, SynthEngine};

use super::{Envelope, EnvelopeCurve};
use super::link::UiEnd;

#[derive(Clone)]
pub struct ControlsState {
    pub delay: StereoSample,
    pub attack: StereoSample,
    pub attack_curve: EnvelopeCurve,
    pub hold: StereoSample,
    pub decay: StereoSample,
    pub decay_curve: EnvelopeCurve,
    pub sustain: StereoSample,
    pub release: StereoSample,
    pub release_curve: EnvelopeCurve,
    pub smooth: StereoSample,
    pub keep_voice_alive: bool,
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
        let env = synth_lock.get_typed_module_mut::<Envelope>(module_id)?;
        let ui_end = env.take_ui_end()?;
        let controls = env.get_controls_state();

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
            Input::Delay => self.controls.delay = value,
            Input::Attack => self.controls.attack = value,
            Input::Hold => self.controls.hold = value,
            Input::Decay => self.controls.decay = value,
            Input::Sustain => self.controls.sustain = value,
            Input::Release => self.controls.release = value,
            _ => (),
        }
    }

    pub fn set_smooth(&mut self, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_smooth(value) {
            self.controls.smooth = value;
        }
    }

    pub fn set_attack_curve(&mut self, curve: EnvelopeCurve) {
        if self.ui_end.as_mut().unwrap().set_attack_curve(curve) {
            self.controls.attack_curve = curve;
        }
    }

    pub fn set_decay_curve(&mut self, curve: EnvelopeCurve) {
        if self.ui_end.as_mut().unwrap().set_decay_curve(curve) {
            self.controls.decay_curve = curve;
        }
    }

    pub fn set_release_curve(&mut self, curve: EnvelopeCurve) {
        if self.ui_end.as_mut().unwrap().set_release_curve(curve) {
            self.controls.release_curve = curve;
        }
    }

    pub fn set_keep_voice_alive(&mut self, value: bool) {
        if self.ui_end.as_mut().unwrap().set_keep_voice_alive(value) {
            self.controls.keep_voice_alive = value;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(env) = synth_lock.get_typed_module_mut::<Envelope>(self.module_id) {
            env.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
