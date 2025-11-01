use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    StereoSample,
    modules::{
        AmplifierConfig, EnvelopeConfig, HarmonicEditorConfig, OscillatorConfig,
        SpectralFilterConfig,
    },
    routing::{ModuleId, ModuleLink},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub last_module_id: ModuleId,
    pub links: Vec<ModuleLink>,
    pub output_level: StereoSample,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            last_module_id: 0,
            links: Default::default(),
            output_level: StereoSample::mono(1.0),
        }
    }
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
