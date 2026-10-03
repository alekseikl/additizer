use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, StereoSample};

#[derive(Clone, Serialize, Deserialize)]
pub struct PitchConfig {
    pub id: ModuleId,
    pub keytrack: bool,
    pub glide_always: bool,
    pub glide_per_octave: bool,
    pub pitch_shift: StereoSample,
    pub glide: StereoSample,
    pub glide_slope: StereoSample,
}

impl Default for PitchConfig {
    fn default() -> Self {
        Self {
            id: -1,
            keytrack: true,
            glide_always: true,
            glide_per_octave: false,
            pitch_shift: 0.0.into(),
            glide: 0.0.into(),
            glide_slope: 0.0.into(),
        }
    }
}
