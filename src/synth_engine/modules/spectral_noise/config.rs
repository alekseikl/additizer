use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, StereoSample};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NoiseColor {
    #[default]
    White,
    Pink,
    Brown,
}

impl NoiseColor {
    pub const ALL: [Self; 3] = [Self::White, Self::Pink, Self::Brown];

    pub fn label(self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Pink => "Pink",
            Self::Brown => "Brown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpectralNoiseConfig {
    pub id: ModuleId,
    pub color: NoiseColor,
    /// Positive: fixed harmonic count. `0`: pitch-based bandlimiting.
    pub bandwidth: i32,
    pub level: StereoSample,
    pub stereo: bool,
    pub amount: StereoSample,
    pub amount_limit_to: StereoSample,
    pub amount_limit_slope: StereoSample,
    pub steal_phase: bool,
}

impl Default for SpectralNoiseConfig {
    fn default() -> Self {
        Self {
            id: -1,
            color: NoiseColor::White,
            bandwidth: 0,
            level: StereoSample::ZERO,
            stereo: true,
            amount: 1.0.into(),
            amount_limit_to: 0.0.into(),
            amount_limit_slope: 0.5.into(),
            steal_phase: false,
        }
    }
}
