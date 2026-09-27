use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, Sample, StereoSample, filters::svf::SvfType};

#[derive(Clone, Serialize, Deserialize)]
pub struct SvfConfig {
    pub id: ModuleId,
    pub filter_type: SvfType,
    /// 0 = absolute cutoff (relative to C4), 1 = full key tracking.
    #[serde(default)]
    pub keytrack: Sample,
    /// Octaves relative to C4.
    pub cutoff: StereoSample,
    /// [0, 1]; 0 = Q of 0.5.
    pub resonance: StereoSample,
    /// dB of input gain into the saturator.
    pub drive: StereoSample,
}

impl Default for SvfConfig {
    fn default() -> Self {
        Self {
            id: -1,
            filter_type: SvfType::default(),
            keytrack: 0.0,
            cutoff: 3.0.into(),
            resonance: 0.0.into(),
            drive: 0.0.into(),
        }
    }
}
