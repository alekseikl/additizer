use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::synth_engine::ModuleId;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct UiModuleConfig {
    pub id: ModuleId,
    pub label: String,
    #[serde(default)]
    pub grid_x: i32,
    #[serde(default)]
    pub grid_y: i32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub modules: FxHashMap<ModuleId, UiModuleConfig>,
}
