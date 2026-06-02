use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{
    Input, ModuleId, Sample, StereoSample, SynthEngine,
    oscillator::{
        MAX_UNISON_VOICES, Oscillator, PhasesDst,
        link::{UiEnd, UiUpdate},
    },
};

#[derive(Clone)]
pub struct UnisonUiState {
    pub initial_phase: StereoSample,
    pub phase_shift: StereoSample,
    pub phase_shift_to: StereoSample,
    pub gain: StereoSample,
    pub gain_to: StereoSample,
}

#[derive(Clone)]
pub struct UiState {
    pub gain: StereoSample,
    pub pitch_shift: StereoSample,
    pub detune: StereoSample,
    pub detune_power: StereoSample,
    pub glide: StereoSample,
    pub glide_slope: StereoSample,
    pub phase_shift: StereoSample,
    pub frequency_shift: StereoSample,
    pub unison: usize,
    pub steal_phase: bool,
    pub phases_blend: StereoSample,
    pub gains_blend: StereoSample,
    pub unison_params: [UnisonUiState; MAX_UNISON_VOICES],
}

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    controls: UiState,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let osc = synth_lock.get_typed_module_mut::<Oscillator>(module_id)?;
        let ui_end = osc.take_ui_end()?;
        let controls = osc.get_ui_state();

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

    pub fn sync(&mut self) {
        let synth_lock = self.synth.lock();

        if let Some(osc) = synth_lock.get_typed_module::<Oscillator>(self.module_id) {
            self.controls = osc.get_ui_state();
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

    pub fn controls(&self) -> &UiState {
        &self.controls
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_param(input, value) {
            match input {
                Input::Gain => self.controls.gain = value,
                Input::PitchShift => self.controls.pitch_shift = value,
                Input::PhaseShift => self.controls.phase_shift = value,
                Input::FrequencyShift => self.controls.frequency_shift = value,
                Input::Detune => self.controls.detune = value,
                Input::DetunePower => self.controls.detune_power = value,
                Input::Glide => self.controls.glide = value,
                Input::GlideSlope => self.controls.glide_slope = value,
                Input::PhasesBlend => self.controls.phases_blend = value,
                Input::GainsBlend => self.controls.gains_blend = value,
                _ => (),
            }
        }
    }

    pub fn set_unison(&mut self, unison: usize) {
        if self.ui_end.as_mut().unwrap().set_unison(unison) {
            self.controls.unison = unison;
        }
    }

    pub fn set_steal_phase(&mut self, steal_phase: bool) {
        if self.ui_end.as_mut().unwrap().set_steal_phase(steal_phase) {
            self.controls.steal_phase = steal_phase;
        }
    }

    pub fn set_unison_initial_phase(&mut self, idx: usize, value: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_unison_initial_phase(idx, value)
        {
            self.controls.unison_params[idx].initial_phase = value;
        }
    }

    pub fn set_unison_phase_shift(&mut self, idx: usize, value: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_unison_phase_shift(idx, value)
        {
            self.controls.unison_params[idx].phase_shift = value;
        }
    }

    pub fn set_unison_phase_shift_to(&mut self, idx: usize, value: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_unison_phase_shift_to(idx, value)
        {
            self.controls.unison_params[idx].phase_shift_to = value;
        }
    }

    pub fn set_unison_gain(&mut self, idx: usize, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_unison_gain(idx, value) {
            self.controls.unison_params[idx].gain = value;
        }
    }

    pub fn set_unison_gain_to(&mut self, idx: usize, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_unison_gain_to(idx, value) {
            self.controls.unison_params[idx].gain_to = value;
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
