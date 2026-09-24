use serde::{Deserialize, Deserializer, Serialize, de::Error};

use crate::synth_engine::{
    ModuleId, Sample, StereoSample,
    filters::spectral_filter::FilterType,
    spectral_filter::{MAX_DRIVE, MIN_CUTOFF, MIN_DRIVE, q_from_resonance},
};

pub const MAX_EQ_FILTERS: usize = 16;
pub const MIN_CUTOFF_HZ: Sample = 16.0;
pub const MAX_CUTOFF_HZ: Sample = 33_000.0;
pub const MIN_Q: Sample = 0.02;
pub const MAX_Q: Sample = 40.0;

/// Per-band settings.
#[derive(Clone, Copy, PartialEq, Serialize)]
pub struct EqFilter {
    pub filter_type: FilterType,
    pub cutoff_hz: Sample,
    pub q: Sample,
    pub drive: Sample,
}

impl Default for EqFilter {
    fn default() -> Self {
        Self {
            filter_type: FilterType::LowPass12,
            cutoff_hz: 1_000.0,
            q: q_from_resonance(0.0),
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

impl<'de> Deserialize<'de> for EqFilter {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            filter_type: FilterType,
            cutoff_hz: Sample,
            #[serde(default)]
            q: Option<Sample>,
            #[serde(default)]
            resonance: Option<Sample>,
            drive: Sample,
        }

        let raw = Raw::deserialize(deserializer)?;
        let q = match (raw.q, raw.resonance) {
            (Some(q), _) => q,
            (None, Some(resonance)) => q_from_resonance(resonance),
            (None, None) => return Err(D::Error::missing_field("q")),
        };

        Ok(Self {
            filter_type: raw.filter_type,
            cutoff_hz: raw.cutoff_hz,
            q,
            drive: raw.drive,
        })
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
