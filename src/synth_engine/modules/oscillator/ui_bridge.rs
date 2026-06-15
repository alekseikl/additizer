use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{
    Input, ModuleHandle, ModuleId, Sample, StereoSample, SynthEngine, synth_module::ModuleUiBridge,
};

use super::{
    Oscillator, OscillatorConfig, PhasesDst,
    link::{UiEnd, UiUpdate},
};

pub struct OscillatorUiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: UiEnd,
    config: OscillatorConfig,
}

impl OscillatorUiBridge {
    pub fn try_new(
        module_id: ModuleId,
        synth: Arc<Mutex<SynthEngine>>,
        osc: &mut Oscillator,
    ) -> Option<Self> {
        Some(Self {
            synth,
            module_id,
            ui_end: osc.ui_end.take()?,
            config: osc.get_config(),
        })
    }

    pub fn sync(&mut self) {
        let synth_lock = self.synth.lock();

        if let Some(ModuleHandle::Oscillator(osc)) = synth_lock.get_module(self.module_id) {
            self.config = osc.get_config();
        }
    }

    pub fn config(&self) -> &OscillatorConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.set_param(input, value) {
            match input {
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

    pub fn set_unison_initial_phase(&mut self, idx: usize, value: StereoSample) {
        if self.ui_end.set_unison_initial_phase(idx, value) {
            self.config.unison[idx].initial_phase = value;
        }
    }

    pub fn set_unison_phase_shift(&mut self, idx: usize, value: StereoSample) {
        if self.ui_end.set_unison_phase_shift(idx, value) {
            self.config.unison[idx].phase_shift = value;
        }
    }

    pub fn set_unison_phase_shift_to(&mut self, idx: usize, value: StereoSample) {
        if self.ui_end.set_unison_phase_shift_to(idx, value) {
            self.config.unison[idx].phase_shift_to = value;
        }
    }

    pub fn set_unison_gain(&mut self, idx: usize, value: StereoSample) {
        if self.ui_end.set_unison_gain(idx, value) {
            self.config.unison[idx].gain = value;
        }
    }

    pub fn set_unison_gain_to(&mut self, idx: usize, value: StereoSample) {
        if self.ui_end.set_unison_gain_to(idx, value) {
            self.config.unison[idx].gain_to = value;
        }
    }

    pub fn apply_unison_level_shape(
        &mut self,
        center: StereoSample,
        level: StereoSample,
        to: bool,
    ) {
        self.ui_end.apply_unison_level_shape(center, level, to);
    }

    pub fn randomize_phases(
        &mut self,
        from: Sample,
        to: Sample,
        stereo_spread: Sample,
        dst: PhasesDst,
    ) {
        self.ui_end.randomize_phases(from, to, stereo_spread, dst);
    }
}

impl ModuleUiBridge for OscillatorUiBridge {
    fn update(&mut self) {
        while let Some(update) = self.ui_end.pop_update() {
            match update {
                UiUpdate::RefreshState => {
                    self.sync();
                }
            }
        }
    }
}
