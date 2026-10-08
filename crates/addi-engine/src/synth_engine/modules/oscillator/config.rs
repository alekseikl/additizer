use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    synth_engine::from_st,
    synth_engine::{ModuleId, Sample, StereoSample, oscillator::MAX_UNISON_VOICES},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnisonStyle {
    #[default]
    #[serde(alias = "Custom")]
    Manual,
    #[serde(alias = "Style1", alias = "SynthFlat")]
    Flat,
    #[serde(alias = "SynthConvex")]
    Convex,
}

fn default_unison_stereo() -> Sample {
    1.0
}

fn deserialize_phase_random_stereo<'de, D>(deserializer: D) -> Result<Sample, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrSample {
        Sample(Sample),
        Bool(bool),
    }

    match BoolOrSample::deserialize(deserializer)? {
        BoolOrSample::Sample(value) => Ok(value),
        BoolOrSample::Bool(true) => Ok(1.0),
        BoolOrSample::Bool(false) => Ok(0.0),
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct UnisonConfig {
    pub initial_phase: StereoSample,
    pub phase_shift: StereoSample,
    pub phase_shift_to: StereoSample,
    pub gain: StereoSample,
    pub gain_to: StereoSample,
}

impl Default for UnisonConfig {
    fn default() -> Self {
        Self {
            initial_phase: 0.0.into(),
            phase_shift: 0.0.into(),
            phase_shift_to: 0.0.into(),
            gain: 1.0.into(),
            gain_to: 1.0.into(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OscillatorConfig {
    pub id: ModuleId,
    pub unison_voices: usize,
    #[serde(default)]
    pub unison_style: UnisonStyle,
    /// Width of Flat and Convex unison across the channels. `0` mixes both channels
    /// together; `1` leaves each detuned oscillator on one channel.
    #[serde(default = "default_unison_stereo")]
    pub unison_stereo: Sample,
    pub steal_phase: bool,
    #[serde(default)]
    pub phase_random: Sample,
    /// Extra random offset of each channel away from the shared random phases.
    /// `0` keeps both channels on those phases; `1` offsets each by up to half a cycle.
    /// Older presets stored this as a bool; `true` loads as `1`.
    #[serde(default, deserialize_with = "deserialize_phase_random_stereo")]
    pub phase_random_stereo: Sample,
    #[serde(default)]
    pub mono_spectrum: bool,
    pub detune: StereoSample,
    #[serde(alias = "detune_power")]
    pub detune_focus: StereoSample,
    pub phase_shift: StereoSample,
    pub frequency_shift: StereoSample,
    pub phases_blend: StereoSample,
    pub gains_blend: StereoSample,
    pub unison: [UnisonConfig; MAX_UNISON_VOICES],
}

impl Default for OscillatorConfig {
    fn default() -> Self {
        Self {
            id: -1,
            unison_voices: 1,
            unison_style: UnisonStyle::Manual,
            unison_stereo: default_unison_stereo(),
            steal_phase: false,
            phase_random: 0.0,
            phase_random_stereo: 0.0,
            mono_spectrum: false,
            detune: from_st(0.2).into(),
            detune_focus: 0.0.into(),
            phase_shift: 0.0.into(),
            frequency_shift: 0.0.into(),
            phases_blend: 0.0.into(),
            gains_blend: 0.8.into(),
            unison: Default::default(),
        }
    }
}
