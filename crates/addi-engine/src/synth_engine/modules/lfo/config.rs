use serde::{Deserialize, Deserializer, Serialize};

use addi_dsp::segments::Segments;

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

fn deserialize_optional_skew<'de, D>(deserializer: D) -> Result<Option<Smp>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_skew(deserializer).map(Some)
}

fn default_frequency() -> StereoSample {
    1.0.into()
}

#[derive(Clone, Serialize)]
pub struct LfoConfig {
    pub id: ModuleId,
    pub steal_phase: bool,
    pub frequency: StereoSample,
    pub phase_shift: StereoSample,
    pub segments: Segments,
}

impl Default for LfoConfig {
    fn default() -> Self {
        Self {
            id: -1,
            steal_phase: false,
            frequency: 1.0.into(),
            phase_shift: 0.0.into(),
            segments: Segments::triangle(),
        }
    }
}

#[derive(Deserialize)]
struct LfoConfigDe {
    id: ModuleId,
    #[serde(default)]
    steal_phase: bool,
    #[serde(default = "default_frequency")]
    frequency: StereoSample,
    #[serde(default)]
    phase_shift: StereoSample,
    #[serde(default)]
    segments: Option<Segments>,
    /// Presets saved when this field was named `line`.
    #[serde(default)]
    line: Option<Segments>,
    #[serde(default)]
    shape: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_skew")]
    skew: Option<Smp>,
}

impl From<LfoConfigDe> for LfoConfig {
    fn from(raw: LfoConfigDe) -> Self {
        let segments = raw
            .segments
            .or(raw.line)
            .unwrap_or_else(|| legacy_segments(raw.shape.as_deref(), raw.skew));

        Self {
            id: raw.id,
            steal_phase: raw.steal_phase,
            frequency: raw.frequency,
            phase_shift: raw.phase_shift,
            segments,
        }
    }
}

impl<'de> Deserialize<'de> for LfoConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        LfoConfigDe::deserialize(deserializer).map(Self::from)
    }
}

fn legacy_segments(shape: Option<&str>, skew: Option<Smp>) -> Segments {
    let skew = skew.unwrap_or(0.5);

    match shape.unwrap_or("Triangle") {
        "Square" => Segments::square_at(skew),
        "Sine" => Segments::sine_at(skew),
        _ => Segments::triangle_at(skew),
    }
}
