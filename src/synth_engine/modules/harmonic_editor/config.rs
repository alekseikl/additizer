use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    ModuleId, Sample,
    buffer::{HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE},
    routing::NUM_CHANNELS,
    types::ComplexSample,
};

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct ComplexCfg {
    pub re: Sample,
    pub im: Sample,
}

impl ComplexCfg {
    pub fn from_complex(complex: ComplexSample) -> Self {
        Self {
            re: complex.re,
            im: complex.im,
        }
    }

    pub fn complex(&self) -> ComplexSample {
        ComplexSample::new(self.re, self.im)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub id: ModuleId,
    pub spectrum: [Vec<ComplexCfg>; NUM_CHANNELS],
}

impl Default for Config {
    fn default() -> Self {
        let mut cfg = Self {
            id: -1,
            spectrum: Default::default(),
        };

        let harmonic_series = &HARMONIC_SERIES_BUFFER;

        for channel in &mut cfg.spectrum {
            channel.extend(
                harmonic_series
                    .iter()
                    .take(SPECTRAL_BUFFER_SIZE)
                    .map(|c| ComplexCfg::from_complex(*c)),
            );
        }

        cfg
    }
}
