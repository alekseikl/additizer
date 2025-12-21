use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    BUFFER_SIZE, VoiceOverride,
    modules::{
        AmplifierConfig, EnvelopeConfig, ExternalParamConfig, LfoConfig, ModulationFilterConfig,
        OscillatorConfig, OutputConfig, SpectralBlendConfig, SpectralFilterConfig,
        SpectralMixerConfig, harmonic_editor::HarmonicEditorConfig,
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
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            next_module_id: MIN_MODULE_ID,
            num_voices: MAX_VOICES / 4,
            voice_override: VoiceOverride::Kill,
            buffer_size: BUFFER_SIZE,
            links: Default::default(),
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
    SpectralMixer(CfgBox<SpectralMixerConfig>),
    HarmonicEditor(CfgBox<HarmonicEditorConfig>),
    ExternalParam(CfgBox<ExternalParamConfig>),
    ModulationFilter(CfgBox<ModulationFilterConfig>),
    Lfo(CfgBox<LfoConfig>),
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Config {
    pub routing: CfgBox<RoutingConfig>,
    pub modules: CfgBox<HashMap<ModuleId, ModuleConfig>>,
    pub output: CfgBox<OutputConfig>,
}
