use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ModuleId, Sample, StereoSample, oscillator::MAX_UNISON_VOICES},
    utils::st_to_octave,
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
pub struct Config {
    pub id: ModuleId,
    pub unison_voices: usize,
    pub steal_phase: bool,
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

impl Default for Config {
    fn default() -> Self {
        static INITIAL_PHASES: [Sample; MAX_UNISON_VOICES] = [
            0.0, 0.9068176, 0.6544455, 0.26577616, 0.24667478, 0.12834072, 0.5805929, 0.55541587,
            0.58291245, 0.03298676, 0.8845756, 0.96093744, 0.42001683, 0.63606197, 0.28810132,
            0.5167134,
        ];

        let mut unison = <[UnisonConfig; MAX_UNISON_VOICES]>::default();

        for (voice, phase) in unison.iter_mut().zip(&INITIAL_PHASES) {
            voice.initial_phase = StereoSample::splat(*phase);
        }

        Self {
            id: -1,
            unison_voices: 1,
            steal_phase: false,
            gain: 1.0.into(),
            pitch_shift: 0.0.into(),
            detune: st_to_octave(0.2).into(),
            detune_power: 0.0.into(),
            glide: 0.0.into(),
            glide_slope: 0.0.into(),
            phase_shift: 0.0.into(),
            frequency_shift: 0.0.into(),
            phases_blend: 0.0.into(),
            gains_blend: 0.0.into(),
            unison,
        }
    }
}
