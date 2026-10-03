use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, SPECTRAL_BUFFER_SIZE, Sample};

pub const MIN_HARMONIC: u16 = 1;
pub const MAX_HARMONIC: u16 = (SPECTRAL_BUFFER_SIZE - 1) as u16;
/// Exclusive end of a harmonic range. One past the last bin.
pub const MAX_HARMONIC_END: u16 = SPECTRAL_BUFFER_SIZE as u16;
pub const MIN_BAND_HZ: Sample = 16.0;
pub const MAX_BAND_HZ: Sample = 33_000.0;

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BandSelectMode {
    #[default]
    Harmonic,
    Frequency,
}

impl BandSelectMode {
    pub const ALL: [Self; 2] = [Self::Harmonic, Self::Frequency];

    pub fn label(self) -> &'static str {
        match self {
            Self::Harmonic => "Harmonic",
            Self::Frequency => "Frequency",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectralBandSelectConfig {
    pub id: ModuleId,
    pub mode: BandSelectMode,
    pub harmonic_from: u16,
    pub harmonic_to: u16,
    pub freq_from: Sample,
    pub freq_to: Sample,
}

impl Default for SpectralBandSelectConfig {
    fn default() -> Self {
        Self {
            id: -1,
            mode: BandSelectMode::Harmonic,
            harmonic_from: MIN_HARMONIC,
            harmonic_to: MAX_HARMONIC_END,
            freq_from: MIN_BAND_HZ,
            freq_to: MAX_BAND_HZ,
        }
    }
}

impl SpectralBandSelectConfig {
    pub fn sanitized(self) -> Self {
        Self {
            harmonic_from: self.harmonic_from.clamp(MIN_HARMONIC, MAX_HARMONIC),
            harmonic_to: self.harmonic_to.clamp(MIN_HARMONIC, MAX_HARMONIC_END),
            freq_from: self.freq_from.clamp(MIN_BAND_HZ, MAX_BAND_HZ),
            freq_to: self.freq_to.clamp(MIN_BAND_HZ, MAX_BAND_HZ),
            ..self
        }
    }
}
