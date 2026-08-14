use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ModuleId, Sample, StereoSample, oscillator::MAX_UNISON_VOICES},
    utils::from_st,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct UnisonConfig {
    pub initial_phase: StereoSample,
    pub phase_shift: StereoSample,
    pub phase_shift_to: StereoSample,
    pub gain: StereoSample,
    pub gain_to: StereoSample,
}

impl Default for UnisonConfig {
    fn default() -> Self {
        Self {
            initial_phase: 0.0.into(),
            phase_shift: 0.0.into(),
            phase_shift_to: 0.0.into(),
            gain: 1.0.into(),
            gain_to: 1.0.into(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OscillatorConfig {
    pub id: ModuleId,
    pub unison_voices: usize,
    pub steal_phase: bool,
    #[serde(default)]
    pub phase_random: Sample,
    #[serde(default)]
    pub pan: StereoSample,
    pub gain: StereoSample,
    pub pitch_shift: StereoSample,
    pub detune: StereoSample,
    pub detune_power: StereoSample,
    pub glide: StereoSample,
    pub glide_slope: StereoSample,
    pub phase_shift: StereoSample,
    pub frequency_shift: StereoSample,
    pub phases_blend: StereoSample,
    pub gains_blend: StereoSample,
    pub unison: [UnisonConfig; MAX_UNISON_VOICES],
}

impl Default for OscillatorConfig {
    fn default() -> Self {
        Self {
            id: -1,
            unison_voices: 1,
            steal_phase: false,
            phase_random: 0.0,
            pan: 0.0.into(),
            gain: 1.0.into(),
            pitch_shift: 0.0.into(),
            detune: from_st(0.2).into(),
            detune_power: 0.0.into(),
            glide: 0.0.into(),
            glide_slope: 0.0.into(),
            phase_shift: 0.0.into(),
            frequency_shift: 0.0.into(),
            phases_blend: 0.0.into(),
            gains_blend: 0.0.into(),
            unison: Default::default(),
        }
    }
}
