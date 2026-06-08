use serde::{Deserialize, Serialize};

use crate::synth_engine::{EngineConfig, ui_bridge::ui_config::UiConfig};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub engine: EngineConfig,
    pub ui: UiConfig,
}
