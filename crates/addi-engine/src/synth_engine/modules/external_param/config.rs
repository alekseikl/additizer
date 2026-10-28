use serde::{Deserialize, Serialize};

use crate::synth_engine::ModuleId;

#[derive(Clone, Serialize, Deserialize)]
pub struct ExternalParamConfig {
    pub id: ModuleId,
    pub selected_param_index: usize,
    pub sample_on_trigger: bool,
    #[serde(default)]
    pub polyphonic: bool,
}

impl Default for ExternalParamConfig {
    fn default() -> Self {
        Self {
            id: -1,
            selected_param_index: 0,
            sample_on_trigger: false,
            polyphonic: false,
        }
    }
}
