use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    modules::{
        AmplifierConfig, EnvelopeConfig, HarmonicEditorConfig, OscillatorConfig,
        SpectralFilterConfig,
    },
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
    HarmonicEditor(Arc<Mutex<HarmonicEditorConfig>>),
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Config {
    pub routing: Arc<Mutex<RoutingConfig>>,
    pub modules: Arc<Mutex<HashMap<ModuleId, ModuleConfig>>>,
}
