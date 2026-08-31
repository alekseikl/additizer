use crate::synth_engine::{
    DisplaySpectrum, Input, Sample, StereoSample, synth_module::ModuleUiBridge,
};

use super::{
    Oscillator, OscillatorConfig, PhasesDst,
    link::{UiEnd, Unison},
};

pub struct OscillatorUiBridge {
    ui_end: UiEnd,
    config: OscillatorConfig,
}

impl OscillatorUiBridge {
    pub fn try_new(osc: &mut Oscillator) -> Option<Self> {
        Some(Self {
            ui_end: osc.ui_end.take()?,
            config: osc.get_config(),
        })
    }

    pub fn config(&self) -> &OscillatorConfig {
        &self.config
    }

    pub fn unison_mut(&mut self) -> &mut Unison {
        self.ui_end.get_unison_mut()
    }

    pub fn get_spectrum(&mut self) -> &DisplaySpectrum {
        self.ui_end.get_spectrum()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.set_param(input, value) {
            match input {
                Input::Pan => self.config.pan = value,
                Input::Gain => self.config.gain = value,
                Input::PitchShift => self.config.pitch_shift = value,
                Input::PhaseShift => self.config.phase_shift = value,
                Input::FrequencyShift => self.config.frequency_shift = value,
                Input::Detune => self.config.detune = value,
                Input::DetunePower => self.config.detune_power = value,
                Input::Glide => self.config.glide = value,
                Input::GlideSlope => self.config.glide_slope = value,
                Input::PhasesBlend => self.config.phases_blend = value,
                Input::GainsBlend => self.config.gains_blend = value,
                _ => (),
            }
        }
    }

    pub fn set_unison(&mut self, unison: usize) {
        if self.ui_end.set_unison(unison) {
            self.config.unison_voices = unison;
        }
    }

    pub fn set_steal_phase(&mut self, steal_phase: bool) {
        if self.ui_end.set_steal_phase(steal_phase) {
            self.config.steal_phase = steal_phase;
        }
    }

    pub fn set_phase_random(&mut self, phase_random: Sample) {
        if self.ui_end.set_phase_random(phase_random) {
            self.config.phase_random = phase_random;
        }
    }

    pub fn set_unison_initial_phase(&mut self, idx: usize, value: StereoSample) {
        self.ui_end.set_unison_initial_phase(idx, value);
    }

    pub fn set_unison_phase_shift(&mut self, idx: usize, value: StereoSample) {
        self.ui_end.set_unison_phase_shift(idx, value);
    }

    pub fn set_unison_phase_shift_to(&mut self, idx: usize, value: StereoSample) {
        self.ui_end.set_unison_phase_shift_to(idx, value);
    }

    pub fn set_unison_gain(&mut self, idx: usize, value: StereoSample) {
        self.ui_end.set_unison_gain(idx, value);
    }

    pub fn set_unison_gain_to(&mut self, idx: usize, value: StereoSample) {
        self.ui_end.set_unison_gain_to(idx, value);
    }

    pub fn apply_unison_level_shape(
        &mut self,
        center: StereoSample,
        level: StereoSample,
        to: bool,
    ) {
        self.ui_end.apply_unison_level_shape(center, level, to);
    }

    pub fn randomize_phases(&mut self, amount: Sample, stereo_spread: Sample, dst: PhasesDst) {
        self.ui_end.randomize_phases(amount, stereo_spread, dst);
    }
}

impl ModuleUiBridge for OscillatorUiBridge {
    fn update(&mut self) -> bool {
        false
    }
}
