use crate::ui_bridge::ModuleUiBridge;
use addi_engine::oscillator::{
    Oscillator, OscillatorConfig, OscillatorLinks, OscillatorUiEnd, PhasesDst, Unison,
};
use addi_engine::{DisplaySpectrum, Input, Sample, StereoSample};

pub struct OscillatorUiBridge<L: OscillatorLinks = crate::links::oscillator::Links> {
    ui_end: L::UiEnd,
    config: OscillatorConfig,
}

impl<L: OscillatorLinks> OscillatorUiBridge<L> {
    pub fn try_new(osc: &mut Oscillator<L>) -> Option<Self> {
        Some(Self {
            ui_end: osc.take_ui_end()?,
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
                Input::PhaseShift => self.config.phase_shift = value,
                Input::FrequencyShift => self.config.frequency_shift = value,
                Input::Detune => self.config.detune = value,
                Input::DetuneFocus => self.config.detune_focus = value,
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

    pub fn set_phase_random_stereo(&mut self, phase_random_stereo: bool) {
        if self.ui_end.set_phase_random_stereo(phase_random_stereo) {
            self.config.phase_random_stereo = phase_random_stereo;
        }
    }

    pub fn set_mono_spectrum(&mut self, mono_spectrum: bool) {
        if self.ui_end.set_mono_spectrum(mono_spectrum) {
            self.config.mono_spectrum = mono_spectrum;
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

impl<L: OscillatorLinks> ModuleUiBridge for OscillatorUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
