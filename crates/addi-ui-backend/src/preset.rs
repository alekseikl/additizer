use serde::{Deserialize, Serialize};

use addi_engine::EngineConfig;

use crate::ui_bridge::ui_config::UiConfig;

fn default_oversampling() -> bool {
    true
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PresetInfo {
    pub title: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Preset {
    #[serde(flatten)]
    pub info: PresetInfo,
    pub engine: EngineConfig,
    #[serde(default = "default_oversampling")]
    pub oversampling: bool,
    pub ui: UiConfig,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            info: PresetInfo::default(),
            engine: EngineConfig::default(),
            oversampling: default_oversampling(),
            ui: UiConfig::default(),
        }
    }
}
