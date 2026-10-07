use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::from_st,
    synth_engine::{ModuleId, Sample, StereoSample, oscillator::MAX_UNISON_VOICES},
};

/// How unison voice levels are produced.
///
/// `Custom` uses the per-voice level sliders. `Style1` pairs voices around
/// the detune range. The center (or the inner pair, when the count is even)
/// and the detuned voices use two shared levels, then `unison_stereo`
/// crossfades the two channels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnisonStyle {
    #[default]
    Custom,
    #[serde(alias = "Vital")]
    Style1,
}

fn default_unison_stereo() -> Sample {
    1.0
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
    /// Width of Style1 unison across the channels. `0` mixes both channels
    /// together; `1` leaves each detuned oscillator on one channel.
    #[serde(default = "default_unison_stereo")]
    pub unison_stereo: Sample,
    pub steal_phase: bool,
    #[serde(default)]
    pub phase_random: Sample,
    /// When set, each channel draws its own random phases. When unset, both channels share them.
    #[serde(default)]
    pub phase_random_stereo: bool,
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
            unison_style: UnisonStyle::Custom,
            unison_stereo: default_unison_stereo(),
            steal_phase: false,
            phase_random: 0.0,
            phase_random_stereo: false,
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
