use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    MAX_BLOCK_SIZE,
    modules::OutputConfig,
    routing::{MAX_VOICES, MIN_MODULE_ID, ModuleId, ModuleLink},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub next_module_id: ModuleId,
    pub num_voices: usize,
    pub legato: bool,
    pub block_size: usize,
    pub oversampling: bool,
    pub stereo_spectrum: bool,
    pub links: Vec<ModuleLink>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            next_module_id: MIN_MODULE_ID,
            num_voices: MAX_VOICES / 4,
            legato: false,
            block_size: MAX_BLOCK_SIZE,
            oversampling: false,
            stereo_spectrum: true,
            links: Default::default(),
        }
    }
}

type CfgBox<T> = Arc<Mutex<T>>;

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Config {
    pub routing: RoutingConfig,
    pub output: CfgBox<OutputConfig>,
}
