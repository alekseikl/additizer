use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ModuleId, Sample, StereoSample},
    utils::from_ms,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct EnvelopeConfig {
    pub id: ModuleId,
    pub keep_voice_alive: bool,
    #[serde(default)]
    pub steal_level: bool,
    pub delay: StereoSample,
    pub attack: StereoSample,
    #[serde(alias = "attack_curvature")]
    pub attack_slope: Sample,
    pub hold: StereoSample,
    pub decay: StereoSample,
    #[serde(alias = "decay_curvature")]
    pub decay_slope: Sample,
    pub sustain: StereoSample,
    pub release: StereoSample,
    #[serde(alias = "release_curvature")]
    pub release_slope: Sample,
}

impl Default for EnvelopeConfig {
    fn default() -> Self {
        Self {
            id: -1,
            keep_voice_alive: false,
            steal_level: false,
            delay: 0.0.into(),
            attack: 0.0.into(),
            attack_slope: 0.3,
            hold: 0.0.into(),
            decay: from_ms(200.0).into(),
            decay_slope: 0.2,
            sustain: 1.0.into(),
            release: from_ms(300.0).into(),
            release_slope: 0.2,
        }
    }
}
