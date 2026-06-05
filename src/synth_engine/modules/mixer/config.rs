use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, StereoSample, VolumeType};

pub const MAX_INPUTS: u8 = 6;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct InputConfig {
    pub volume_type: VolumeType,
    pub level: StereoSample,
    pub gain: StereoSample,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            volume_type: VolumeType::default(),
            level: StereoSample::ZERO,
            gain: StereoSample::ONE,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub id: ModuleId,
    pub num_inputs: u8,
    pub inputs: [InputConfig; MAX_INPUTS as usize],
    pub output_volume_type: VolumeType,
    pub output_level: StereoSample,
    pub output_gain: StereoSample,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            id: -1,
            num_inputs: 2,
            inputs: Default::default(),
            output_volume_type: VolumeType::Gain,
            output_level: 0.0.into(),
            output_gain: 1.0.into(),
        }
    }
}
