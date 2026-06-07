use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, StereoSample};

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SpectralFilterType {
    #[default]
    LowPass,
    HighPass,
    BandPass,
    BandStop,
    Peaking,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectralFilterConfig {
    pub id: ModuleId,
    pub filter_type: SpectralFilterType,
    pub fourth_order: bool,
    pub linear_phase: bool,
    pub cutoff: StereoSample,
    pub q: StereoSample,
    pub drive: StereoSample,
}

impl Default for SpectralFilterConfig {
    fn default() -> Self {
        Self {
            id: -1,
            filter_type: SpectralFilterType::default(),
            fourth_order: false,
            linear_phase: true,
            cutoff: 1.0.into(),
            q: 0.7.into(),
            drive: 0.0.into(),
        }
    }
}
