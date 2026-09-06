use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ModuleId, Sample, StereoSample, filters::spectral_filter::FilterType},
    utils::from_st,
};

fn default_keytrack() -> Sample {
    1.0
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectralFilterConfig {
    pub id: ModuleId,
    pub filter_type: FilterType,
    pub linear_phase: bool,
    #[serde(default = "default_keytrack")]
    pub keytrack: Sample,
    pub q_limit_to: StereoSample,
    pub q_limit_curve: StereoSample,
    pub cutoff: StereoSample,
    pub resonance: StereoSample,
    pub drive: StereoSample,
}

impl Default for SpectralFilterConfig {
    fn default() -> Self {
        Self {
            id: -1,
            filter_type: FilterType::default(),
            linear_phase: false,
            keytrack: 0.0,
            q_limit_to: from_st(12.0).into(),
            q_limit_curve: 0.5.into(),
            cutoff: 1.0.into(),
            resonance: 0.0.into(),
            drive: 0.0.into(),
        }
    }
}
