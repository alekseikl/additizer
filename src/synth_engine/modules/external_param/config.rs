use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ModuleId, Sample},
    utils::from_ms,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct ExternalParamConfig {
    pub id: ModuleId,
    pub selected_param_index: usize,
    pub smooth: Sample,
    pub sample_on_trigger: bool,
    #[serde(default)]
    pub make_bipolar: bool,
}

impl Default for ExternalParamConfig {
    fn default() -> Self {
        Self {
            id: -1,
            selected_param_index: 0,
            smooth: from_ms(2.0),
            sample_on_trigger: false,
            make_bipolar: false,
        }
    }
}
