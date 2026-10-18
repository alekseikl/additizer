use std::array;

use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    ModuleId, Smp,
    buffer::{DC_OFFSET, SPECTRAL_BUFFER_SIZE},
    voices::NUM_CHANNELS,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "HarmonicEditorConfigDe")]
pub struct HarmonicEditorConfig {
    pub id: ModuleId,
    pub amplitudes: [Vec<Smp>; NUM_CHANNELS],
    pub phases: [Vec<Smp>; NUM_CHANNELS],
    /// Positive: fixed harmonic count. `0`: pitch-based bandlimiting.
    #[serde(default)]
    pub bandwidth: i32,
    /// Process left and right independently. When false, only the left channel is rendered.
    pub stereo: bool,
    /// On note-on, replace amplitudes and phases with the spectrum input.
    #[serde(default)]
    pub capture_input: bool,
}

/// Accepts the current `stereo` field and the previous `mono` field (`stereo = !mono`).
#[derive(Deserialize)]
struct HarmonicEditorConfigDe {
    id: ModuleId,
    amplitudes: [Vec<Smp>; NUM_CHANNELS],
    phases: [Vec<Smp>; NUM_CHANNELS],
    #[serde(default)]
    bandwidth: i32,
    #[serde(default)]
    mono: bool,
    #[serde(default)]
    stereo: Option<bool>,
    #[serde(default)]
    capture_input: bool,
}

impl From<HarmonicEditorConfigDe> for HarmonicEditorConfig {
    fn from(raw: HarmonicEditorConfigDe) -> Self {
        Self {
            id: raw.id,
            amplitudes: raw.amplitudes,
            phases: raw.phases,
            bandwidth: raw.bandwidth,
            stereo: raw.stereo.unwrap_or(!raw.mono),
            capture_input: raw.capture_input,
        }
    }
}

impl Default for HarmonicEditorConfig {
    fn default() -> Self {
        let mut amplitudes: [Vec<Smp>; NUM_CHANNELS] =
            array::from_fn(|_| vec![0.0; SPECTRAL_BUFFER_SIZE]);
        let mut phases: [Vec<Smp>; NUM_CHANNELS] =
            array::from_fn(|_| vec![0.0; SPECTRAL_BUFFER_SIZE]);

        for (amplitudes, phases) in amplitudes.iter_mut().zip(phases.iter_mut()) {
            fill_default_harmonics(amplitudes.iter_mut(), phases.iter_mut());
        }

        Self {
            id: -1,
            amplitudes,
            phases,
            bandwidth: 0,
            stereo: true,
            capture_input: false,
        }
    }
}

pub fn sawtooth_phase(harmonic: usize) -> Smp {
    if harmonic & 1 == 0 { 0.25 } else { 0.75 }
}

pub(super) fn fill_default_harmonics<'a>(
    mut amplitudes: impl Iterator<Item = &'a mut Smp>,
    mut phases: impl Iterator<Item = &'a mut Smp>,
) {
    if let (Some(dc_amp), Some(dc_phase)) = (amplitudes.next(), phases.next()) {
        *dc_amp = 0.0;
        *dc_phase = 0.0;
    }

    for (idx, (amp, phase)) in amplitudes.zip(phases).enumerate() {
        let idx = idx + DC_OFFSET;

        *amp = 1.0;
        *phase = sawtooth_phase(idx);
    }
}
