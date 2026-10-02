use serde::{Deserialize, Serialize};

use crate::synth_engine::{ModuleId, Sample, StereoSample};
use additizer_dsp::filters::svf::SvfType;

#[derive(Clone, Serialize, Deserialize)]
pub struct SvfConfig {
    pub id: ModuleId,
    pub filter_type: SvfType,
    #[serde(default)]
    pub keytrack: Sample,
    pub cutoff: StereoSample,
    pub resonance: StereoSample,
    pub drive: StereoSample,
}

impl Default for SvfConfig {
    fn default() -> Self {
        Self {
            id: -1,
            filter_type: SvfType::default(),
            keytrack: 0.0,
            cutoff: 0.0.into(),
            resonance: 0.0.into(),
            drive: 0.0.into(),
        }
    }
}
