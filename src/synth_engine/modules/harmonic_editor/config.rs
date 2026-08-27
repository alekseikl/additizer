use std::array;

use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    ModuleId, Sample,
    buffer::{DC_OFFSET, SPECTRAL_BUFFER_SIZE},
    routing::NUM_CHANNELS,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct HarmonicEditorConfig {
    pub id: ModuleId,
    pub amplitudes: [Vec<Sample>; NUM_CHANNELS],
    pub phases: [Vec<Sample>; NUM_CHANNELS],
}

impl Default for HarmonicEditorConfig {
    fn default() -> Self {
        let mut amplitudes: [Vec<Sample>; NUM_CHANNELS] =
            array::from_fn(|_| vec![0.0; SPECTRAL_BUFFER_SIZE]);
        let mut phases: [Vec<Sample>; NUM_CHANNELS] =
            array::from_fn(|_| vec![0.0; SPECTRAL_BUFFER_SIZE]);

        for (amplitudes, phases) in amplitudes.iter_mut().zip(phases.iter_mut()) {
            fill_default_harmonics(amplitudes.iter_mut(), phases.iter_mut());
        }

        Self {
            id: -1,
            amplitudes,
            phases,
        }
    }
}

pub fn sawtooth_phase(harmonic: usize) -> Sample {
    if harmonic & 1 == 0 { 0.0 } else { 0.5 }
}

pub(super) fn fill_default_harmonics<'a>(
    mut amplitudes: impl Iterator<Item = &'a mut Sample>,
    mut phases: impl Iterator<Item = &'a mut Sample>,
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
