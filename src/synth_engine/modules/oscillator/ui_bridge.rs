use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, ModuleId, Sample, StereoSample, SynthEngine};

use super::{
    Config, Oscillator, PhasesDst,
    link::{UiEnd, UiUpdate},
};

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    config: Config,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let osc = synth_lock.get_typed_module_mut::<Oscillator>(module_id)?;
        let ui_end = osc.take_ui_end()?;
        let config = osc.get_config();

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

    pub fn sync(&mut self) {
        let synth_lock = self.synth.lock();

        if let Some(osc) = synth_lock.get_typed_module::<Oscillator>(self.module_id) {
            self.config = osc.get_config();
        }
    }

    pub fn update(&mut self) {
        while let Some(update) = self.ui_end.as_mut().unwrap().pop_update() {
            match update {
                UiUpdate::RefreshState => {
                    self.sync();
                }
            }
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_param(input, value) {
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
        if self.ui_end.as_mut().unwrap().set_unison(unison) {
            self.config.unison_voices = unison;
        }
    }

    pub fn set_steal_phase(&mut self, steal_phase: bool) {
        if self.ui_end.as_mut().unwrap().set_steal_phase(steal_phase) {
            self.config.steal_phase = steal_phase;
        }
    }

    pub fn set_unison_initial_phase(&mut self, idx: usize, value: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_unison_initial_phase(idx, value)
        {
            self.config.unison[idx].initial_phase = value;
        }
    }

    pub fn set_unison_phase_shift(&mut self, idx: usize, value: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_unison_phase_shift(idx, value)
        {
            self.config.unison[idx].phase_shift = value;
        }
    }

    pub fn set_unison_phase_shift_to(&mut self, idx: usize, value: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_unison_phase_shift_to(idx, value)
        {
            self.config.unison[idx].phase_shift_to = value;
        }
    }

    pub fn set_unison_gain(&mut self, idx: usize, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_unison_gain(idx, value) {
            self.config.unison[idx].gain = value;
        }
    }

    pub fn set_unison_gain_to(&mut self, idx: usize, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_unison_gain_to(idx, value) {
            self.config.unison[idx].gain_to = value;
        }
    }

    pub fn apply_unison_level_shape(
        &mut self,
        center: StereoSample,
        level: StereoSample,
        to: bool,
    ) {
        self.ui_end
            .as_mut()
            .unwrap()
            .apply_unison_level_shape(center, level, to);
    }

    pub fn randomize_phases(
        &mut self,
        from: Sample,
        to: Sample,
        stereo_spread: Sample,
        dst: PhasesDst,
    ) {
        self.ui_end
            .as_mut()
            .unwrap()
            .randomize_phases(from, to, stereo_spread, dst);
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(osc) = synth_lock.get_typed_module_mut::<Oscillator>(self.module_id) {
            osc.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
