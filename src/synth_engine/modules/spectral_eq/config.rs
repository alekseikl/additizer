use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    ModuleId, Sample, StereoSample,
    filters::spectral_filter::{FilterType, MAX_RESONANCE, MIN_RESONANCE},
    spectral_filter::{MAX_DRIVE, MIN_CUTOFF, MIN_DRIVE},
};

pub const MAX_EQ_FILTERS: usize = 16;
pub const MIN_CUTOFF_HZ: Sample = 16.0;
pub const MAX_CUTOFF_HZ: Sample = 33_000.0;

/// Per-band settings.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EqFilter {
    pub filter_type: FilterType,
    pub cutoff_hz: Sample,
    pub resonance: Sample,
    pub drive: Sample,
}

impl Default for EqFilter {
    fn default() -> Self {
        Self {
            filter_type: FilterType::LowPass12,
            cutoff_hz: 1_000.0,
            resonance: 0.0,
            drive: 0.0,
        }
    }
}

impl EqFilter {
    pub fn sanitized(self) -> Self {
        Self {
            filter_type: self.filter_type,
            cutoff_hz: self.cutoff_hz.clamp(MIN_CUTOFF_HZ, MAX_CUTOFF_HZ),
            resonance: self.resonance.clamp(MIN_RESONANCE, MAX_RESONANCE),
            drive: self.drive.clamp(MIN_DRIVE, MAX_DRIVE),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectralEqConfig {
    pub id: ModuleId,
    pub filters: Vec<EqFilter>,
    pub linear_phase: bool,
    pub keytrack: Sample,
    pub q_limit_to: StereoSample,
    pub q_limit_slope: StereoSample,
    pub cutoff: StereoSample,
    pub output_level: StereoSample,
}

impl Default for SpectralEqConfig {
    fn default() -> Self {
        Self {
            id: -1,
            filters: vec![EqFilter::default()],
            linear_phase: false,
            keytrack: 0.0,
            q_limit_to: MIN_CUTOFF.into(),
            q_limit_slope: 0.5.into(),
            cutoff: 0.0.into(),
            output_level: 0.0.into(),
        }
    }
}
