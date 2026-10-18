use serde::{Deserialize, Serialize};

use addi_dsp::filters::{
    control::{MAX_DRIVE, MIN_DRIVE, q_from_resonance},
    spectral_filter::FilterType,
};

use crate::{
    synth_engine::MIN_CUTOFF,
    synth_engine::{
        ModuleId, Smp, StereoSample,
        spectral_filter::{MAX_Q_ROLLOFF, MIN_Q_ROLLOFF},
    },
};

pub const MAX_EQ_FILTERS: usize = 16;
pub const MIN_CUTOFF_HZ: Smp = 16.0;
pub const MAX_CUTOFF_HZ: Smp = 33_000.0;
pub const MIN_Q: Smp = 0.02;
pub const MAX_Q: Smp = 40.0;

/// Per-band settings.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EqFilter {
    pub filter_type: FilterType,
    pub cutoff_hz: Smp,
    pub q: Smp,
    pub drive: Smp,
}

impl Default for EqFilter {
    fn default() -> Self {
        Self {
            filter_type: FilterType::LowPass12,
            cutoff_hz: 1_000.0,
            q: q_from_resonance(0.0).resonant,
            drive: 0.0,
        }
    }
}

impl EqFilter {
    pub fn sanitized(self) -> Self {
        Self {
            filter_type: self.filter_type,
            cutoff_hz: self.cutoff_hz.clamp(MIN_CUTOFF_HZ, MAX_CUTOFF_HZ),
            q: self.q.clamp(MIN_Q, MAX_Q),
            drive: self.drive.clamp(MIN_DRIVE, MAX_DRIVE),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectralEqConfig {
    pub id: ModuleId,
    pub filters: Vec<EqFilter>,
    pub linear_phase: bool,
    pub keytrack: Smp,
    pub q_cutoff: StereoSample,
    pub q_rolloff: StereoSample,
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
            q_cutoff: MIN_CUTOFF.into(),
            q_rolloff: ((MIN_Q_ROLLOFF + MAX_Q_ROLLOFF) * 0.5).into(),
            cutoff: 0.0.into(),
            output_level: 0.0.into(),
        }
    }
}
