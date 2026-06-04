use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, StereoSample};

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ShaperType {
    #[default]
    HardClip,
    Sigmoid,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub id: ModuleId,
    pub shaper_type: ShaperType,
    pub distortion: StereoSample,
    pub clipping_level: StereoSample,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            id: -1,
            shaper_type: ShaperType::default(),
            distortion: 0.0.into(),
            clipping_level: 0.0.into(),
        }
    }
}
