use serde::{Deserialize, Deserializer, Serialize};

use crate::synth_engine::{ModuleId, Smp, StereoSample};

/// Accepts a mono skew, or a legacy stereo value (left channel).
fn deserialize_skew<'de, D>(deserializer: D) -> Result<Smp, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Skew {
        Mono(Smp),
        Stereo { channels: [Smp; 2] },
    }

    match Skew::deserialize(deserializer)? {
        Skew::Mono(value) => Ok(value),
        Skew::Stereo { channels } => Ok(channels[0]),
    }
}

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LfoShape {
    #[default]
    Triangle,
    Square,
    Sine,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LfoConfig {
    pub id: ModuleId,
    pub shape: LfoShape,
    pub steal_phase: bool,
    pub frequency: StereoSample,
    pub phase_shift: StereoSample,
    #[serde(deserialize_with = "deserialize_skew")]
    pub skew: Smp,
}

impl Default for LfoConfig {
    fn default() -> Self {
        Self {
            id: -1,
            shape: LfoShape::default(),
            steal_phase: false,
            frequency: 1.0.into(),
            phase_shift: 0.0.into(),
            skew: 0.5,
        }
    }
}
