use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    synth_engine::from_st,
    synth_engine::{ModuleId, Smp, StereoSample, oscillator::MAX_UNISON_VOICES},
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

fn default_unison_stereo() -> Smp {
    1.0
}

fn default_full_fft_size() -> bool {
    true
}

fn deserialize_phase_random_stereo<'de, D>(deserializer: D) -> Result<Smp, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrSample {
        Sample(Smp),
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
#[serde(from = "OscillatorConfigDe")]
pub struct OscillatorConfig {
    pub id: ModuleId,
    pub unison_voices: usize,
    #[serde(default)]
    pub unison_style: UnisonStyle,
    /// Width of Flat and Convex unison across the channels. `0` mixes both channels
    /// together; `1` leaves each detuned oscillator on one channel.
    #[serde(default = "default_unison_stereo")]
    pub unison_stereo: Smp,
    pub steal_phase: bool,
    #[serde(default)]
    pub phase_random: Smp,
    /// Extra random offset of each channel away from the shared random phases.
    /// `0` keeps both channels on those phases; `1` offsets each by up to half a cycle.
    /// Older presets stored this as a bool; `true` loads as `1`.
    #[serde(default, deserialize_with = "deserialize_phase_random_stereo")]
    pub phase_random_stereo: Smp,
    /// Build each channel from its own spectrum. When false, the right channel
    /// reuses the left waveform.
    pub stereo_spectrum: bool,
    /// Always inverse-FFT the full waveform table. Otherwise a shorter table is
    /// used when the note's cutoff fits inside it.
    #[serde(default = "default_full_fft_size")]
    pub full_fft_size: bool,
    pub detune: StereoSample,
    #[serde(alias = "detune_power")]
    pub detune_focus: StereoSample,
    pub phase_shift: StereoSample,
    pub frequency_shift: StereoSample,
    pub phases_blend: StereoSample,
    pub gains_blend: StereoSample,
    pub unison: [UnisonConfig; MAX_UNISON_VOICES],
}

/// Accepts the current `stereo_spectrum` field and the previous `mono_spectrum`
/// field (`stereo_spectrum = !mono_spectrum`).
#[derive(Deserialize)]
struct OscillatorConfigDe {
    id: ModuleId,
    unison_voices: usize,
    #[serde(default)]
    unison_style: UnisonStyle,
    #[serde(default = "default_unison_stereo")]
    unison_stereo: Smp,
    steal_phase: bool,
    #[serde(default)]
    phase_random: Smp,
    #[serde(default, deserialize_with = "deserialize_phase_random_stereo")]
    phase_random_stereo: Smp,
    #[serde(default)]
    mono_spectrum: bool,
    #[serde(default)]
    stereo_spectrum: Option<bool>,
    #[serde(default = "default_full_fft_size")]
    full_fft_size: bool,
    detune: StereoSample,
    #[serde(alias = "detune_power")]
    detune_focus: StereoSample,
    phase_shift: StereoSample,
    frequency_shift: StereoSample,
    phases_blend: StereoSample,
    gains_blend: StereoSample,
    unison: [UnisonConfig; MAX_UNISON_VOICES],
}

impl From<OscillatorConfigDe> for OscillatorConfig {
    fn from(raw: OscillatorConfigDe) -> Self {
        Self {
            id: raw.id,
            unison_voices: raw.unison_voices,
            unison_style: raw.unison_style,
            unison_stereo: raw.unison_stereo,
            steal_phase: raw.steal_phase,
            phase_random: raw.phase_random,
            phase_random_stereo: raw.phase_random_stereo,
            stereo_spectrum: raw.stereo_spectrum.unwrap_or(!raw.mono_spectrum),
            full_fft_size: raw.full_fft_size,
            detune: raw.detune,
            detune_focus: raw.detune_focus,
            phase_shift: raw.phase_shift,
            frequency_shift: raw.frequency_shift,
            phases_blend: raw.phases_blend,
            gains_blend: raw.gains_blend,
            unison: raw.unison,
        }
    }
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
            stereo_spectrum: true,
            full_fft_size: default_full_fft_size(),
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
