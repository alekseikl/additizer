use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, StereoSample};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub id: ModuleId,
    pub blend: StereoSample,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            id: -1,
            blend: 0.0.into(),
        }
    }
}
