use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    BUFFER_SIZE, StereoSample,
    modules::{
        AmplifierConfig, EnvelopeConfig, ExternalParamConfig, HarmonicEditorConfig,
        OscillatorConfig, SpectralFilterConfig,
    },
    routing::{MAX_VOICES, MIN_MODULE_ID, ModuleId, ModuleLink},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub next_module_id: ModuleId,
    pub num_voices: usize,
    pub buffer_size: usize,
    pub links: Vec<ModuleLink>,
    pub output_level: StereoSample,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            next_module_id: MIN_MODULE_ID,
            num_voices: MAX_VOICES / 2,
            buffer_size: BUFFER_SIZE,
            links: Default::default(),
            output_level: StereoSample::splat(0.25),
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
    ExternalParam(Arc<Mutex<ExternalParamConfig>>),
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Config {
    pub routing: Arc<Mutex<RoutingConfig>>,
    pub modules: Arc<Mutex<HashMap<ModuleId, ModuleConfig>>>,
}
