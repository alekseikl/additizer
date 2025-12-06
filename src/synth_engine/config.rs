use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    BUFFER_SIZE, StereoSample, VoiceOverride,
    modules::{
        AmplifierConfig, EnvelopeConfig, ExternalParamConfig, HarmonicEditorConfig, LfoConfig,
        ModulationFilterConfig, OscillatorConfig, SpectralBlendConfig, SpectralFilterConfig,
    },
    routing::{MAX_VOICES, MIN_MODULE_ID, ModuleId, ModuleLink},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub next_module_id: ModuleId,
    pub num_voices: usize,
    pub voice_override: VoiceOverride,
    pub buffer_size: usize,
    pub links: Vec<ModuleLink>,
    pub output_level: StereoSample,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            next_module_id: MIN_MODULE_ID,
            num_voices: MAX_VOICES / 2,
            voice_override: VoiceOverride::Kill,
            buffer_size: BUFFER_SIZE,
            links: Default::default(),
            output_level: StereoSample::splat(0.25),
        }
    }
}

type CfgBox<T> = Arc<Mutex<T>>;

#[derive(Clone, Serialize, Deserialize)]
pub enum ModuleConfig {
    Envelope(CfgBox<EnvelopeConfig>),
    Amplifier(CfgBox<AmplifierConfig>),
    Oscillator(CfgBox<OscillatorConfig>),
    SpectralFilter(CfgBox<SpectralFilterConfig>),
    SpectralBlend(CfgBox<SpectralBlendConfig>),
    HarmonicEditor(CfgBox<HarmonicEditorConfig>),
    ExternalParam(CfgBox<ExternalParamConfig>),
    ModulationFilter(CfgBox<ModulationFilterConfig>),
    Lfo(CfgBox<LfoConfig>),
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Config {
    pub routing: CfgBox<RoutingConfig>,
    pub modules: CfgBox<HashMap<ModuleId, ModuleConfig>>,
}
