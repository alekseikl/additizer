use serde::{Deserialize, Serialize};

use crate::synth_engine::{EngineConfig, ui_bridge::ui_config::UiConfig};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PresetInfo {
    pub title: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Preset {
    #[serde(flatten)]
    pub info: PresetInfo,
    pub engine: EngineConfig,
    pub ui: UiConfig,
}
