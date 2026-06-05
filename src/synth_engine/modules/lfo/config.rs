use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, StereoSample};

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LfoShape {
    #[default]
    Triangle,
    Square,
    Sine,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub id: ModuleId,
    pub shape: LfoShape,
    pub bipolar: bool,
    pub steal_phase: bool,
    pub frequency: StereoSample,
    pub phase_shift: StereoSample,
    pub skew: StereoSample,
    pub smooth_time: StereoSample,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            id: -1,
            shape: LfoShape::default(),
            bipolar: false,
            steal_phase: false,
            frequency: 1.0.into(),
            phase_shift: 0.0.into(),
            skew: 0.5.into(),
            smooth_time: 0.0.into(),
        }
    }
}
