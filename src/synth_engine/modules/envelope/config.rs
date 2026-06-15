use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ModuleId, Sample, StereoSample},
    utils::from_ms,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct EnvelopeConfig {
    pub id: ModuleId,
    pub keep_voice_alive: bool,
    pub delay: StereoSample,
    pub attack: StereoSample,
    pub attack_curvature: Sample,
    pub hold: StereoSample,
    pub decay: StereoSample,
    pub decay_curvature: Sample,
    pub sustain: StereoSample,
    pub release: StereoSample,
    pub release_curvature: Sample,
    pub smooth: StereoSample,
}

impl Default for EnvelopeConfig {
    fn default() -> Self {
        Self {
            id: -1,
            keep_voice_alive: false,
            delay: 0.0.into(),
            attack: 0.0.into(),
            attack_curvature: 0.3,
            hold: 0.0.into(),
            decay: from_ms(200.0).into(),
            decay_curvature: 0.2,
            sustain: 1.0.into(),
            release: from_ms(300.0).into(),
            release_curvature: 0.2,
            smooth: 0.0.into(),
        }
    }
}
