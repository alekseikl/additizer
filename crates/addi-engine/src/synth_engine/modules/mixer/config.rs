use serde::{Deserialize, Serialize};

use crate::synth_engine::{LevelType, ModuleId, StereoSample};

pub const MAX_INPUTS: u8 = 6;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct InputConfig {
    pub volume_type: LevelType,
    pub level: StereoSample,
    pub gain: StereoSample,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            volume_type: LevelType::default(),
            level: StereoSample::ZERO,
            gain: StereoSample::ONE,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MixerConfig {
    pub id: ModuleId,
    pub num_inputs: u8,
    pub inputs: [InputConfig; MAX_INPUTS as usize],
    pub output_volume_type: LevelType,
    pub output_level: StereoSample,
    pub output_gain: StereoSample,
}

impl Default for MixerConfig {
    fn default() -> Self {
        Self {
            id: -1,
            num_inputs: 2,
            inputs: Default::default(),
            output_volume_type: LevelType::Linear,
            output_level: 0.0.into(),
            output_gain: 1.0.into(),
        }
    }
}
