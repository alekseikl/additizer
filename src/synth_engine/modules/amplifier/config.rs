use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, StereoSample};

#[derive(Clone, Serialize, Deserialize)]
pub struct AmplifierConfig {
    pub id: ModuleId,
    pub gain: StereoSample,
}

impl Default for AmplifierConfig {
    fn default() -> Self {
        Self {
            id: -1,
            gain: 0.0.into(),
        }
    }
}
