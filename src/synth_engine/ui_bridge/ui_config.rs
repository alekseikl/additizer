use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::synth_engine::ModuleId;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct UiModuleConfig {
    pub id: ModuleId,
    pub label: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub modules: FxHashMap<ModuleId, UiModuleConfig>,
}
