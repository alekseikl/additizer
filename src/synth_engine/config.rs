use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    modules::{AmplifierConfig, EnvelopeConfig, OscillatorConfig, SpectralFilterConfig},
    routing::{ModuleId, ModuleLink},
};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub last_module_id: ModuleId,
    pub links: Vec<ModuleLink>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModuleConfig {
    Envelope(Arc<Mutex<EnvelopeConfig>>),
    Amplifier(Arc<Mutex<AmplifierConfig>>),
    Oscillator(Arc<Mutex<OscillatorConfig>>),
    SpectralFilter(Arc<Mutex<SpectralFilterConfig>>),
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    pub routing: Arc<Mutex<RoutingConfig>>,
    pub modules: Arc<Mutex<HashMap<ModuleId, ModuleConfig>>>,
}
