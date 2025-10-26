use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    modules::{AmplifierConfig, EnvelopeConfig, OscillatorConfig, SpectralFilterConfig},
    routing::{ModuleId, ModuleLink, RoutingNode},
};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub last_module_id: ModuleId,
    pub modules: Vec<RoutingNode>,
    pub links: Vec<ModuleLink>,
}

type CfgContainer<T> = Arc<Mutex<HashMap<ModuleId, T>>>;

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    pub routing: Arc<Mutex<RoutingConfig>>,
    pub envelopes: CfgContainer<EnvelopeConfig>,
    pub amplifiers: CfgContainer<AmplifierConfig>,
    pub oscillators: CfgContainer<OscillatorConfig>,
    pub spectral_filters: CfgContainer<SpectralFilterConfig>,
}
