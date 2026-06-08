use crate::synth_engine::{Input, StereoSample, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{Envelope, EnvelopeConfig, EnvelopeCurve};

pub struct EnvelopeUiBridge {
    ui_end: UiEnd,
    config: EnvelopeConfig,
}

impl EnvelopeUiBridge {
    pub fn try_new(env: &mut Envelope) -> Option<Self> {
        Some(Self {
            ui_end: env.ui_end.take()?,
            config: env.get_config(),
        })
    }

    pub fn config(&self) -> &EnvelopeConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::Delay => self.config.delay = value,
            Input::Attack => self.config.attack = value,
            Input::Hold => self.config.hold = value,
            Input::Decay => self.config.decay = value,
            Input::Sustain => self.config.sustain = value,
            Input::Release => self.config.release = value,
            _ => (),
        }
    }

    pub fn set_smooth(&mut self, value: StereoSample) {
        if self.ui_end.set_smooth(value) {
            self.config.smooth = value;
        }
    }

    pub fn set_attack_curve(&mut self, curve: EnvelopeCurve) {
        if self.ui_end.set_attack_curve(curve) {
            self.config.attack_curve = curve;
        }
    }

    pub fn set_decay_curve(&mut self, curve: EnvelopeCurve) {
        if self.ui_end.set_decay_curve(curve) {
            self.config.decay_curve = curve;
        }
    }

    pub fn set_release_curve(&mut self, curve: EnvelopeCurve) {
        if self.ui_end.set_release_curve(curve) {
            self.config.release_curve = curve;
        }
    }

    pub fn set_keep_voice_alive(&mut self, value: bool) {
        if self.ui_end.set_keep_voice_alive(value) {
            self.config.keep_voice_alive = value;
        }
    }
}

impl ModuleUiBridge for EnvelopeUiBridge {
    fn update(&mut self) {}
}
