use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::from_ms,
    synth_engine::{ModuleId, Smp, StereoSample},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct EnvelopeConfig {
    pub id: ModuleId,
    pub keep_voice_alive: bool,
    #[serde(default)]
    pub steal_level: bool,
    pub delay: StereoSample,
    pub attack: StereoSample,
    pub attack_slope: Smp,
    pub hold: StereoSample,
    pub decay: StereoSample,
    pub decay_slope: Smp,
    pub sustain: StereoSample,
    pub release: StereoSample,
    pub release_slope: Smp,
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
