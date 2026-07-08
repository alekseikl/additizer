use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    ModuleId, StereoSample,
    filters::spectral_filter::FilterType,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectralFilterConfig {
    pub id: ModuleId,
    pub filter_type: FilterType,
    pub linear_phase: bool,
    pub cutoff: StereoSample,
    pub resonance: StereoSample,
    pub drive: StereoSample,
}

impl Default for SpectralFilterConfig {
    fn default() -> Self {
        Self {
            id: -1,
            filter_type: FilterType::default(),
            linear_phase: true,
            cutoff: 1.0.into(),
            resonance: 0.0.into(),
            drive: 0.0.into(),
        }
    }
}
