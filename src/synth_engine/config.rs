use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        Input, MAX_BLOCK_SIZE, Sample, StereoSample,
        amplifier::AmplifierConfig,
        envelope::EnvelopeConfig,
        expressions::ExpressionsConfig,
        external_param::ExternalParamConfig,
        harmonic_editor::HarmonicEditorConfig,
        lfo::LfoConfig,
        mixer::MixerConfig,
        oscillator::OscillatorConfig,
        routing::{MAX_VOICES, MIN_MODULE_ID, ModuleId, ModuleLink},
        spectral_blend::SpectralBlendConfig,
        spectral_filter::SpectralFilterConfig,
        spectral_mixer::SpectralMixerConfig,
        wave_shaper::WaveShaperConfig,
    },
    utils::from_ms,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub num_voices: usize,
    pub legato: bool,
    pub block_size: usize,
    pub oversampling: bool,
    pub stereo_spectrum: bool,
    pub voice_kill_time: Sample,
    pub output_gain: StereoSample,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            num_voices: 1,
            legato: false,
            block_size: MAX_BLOCK_SIZE,
            oversampling: false,
            stereo_spectrum: true,
            voice_kill_time: from_ms(30.0),
            output_gain: 1.0.into(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LinkConfig {
    pub src_id: ModuleId,
    pub dst_id: ModuleId,
    pub dst_input: Input,
    pub amount: StereoSample,
    pub modulator_id: Option<ModuleId>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ModuleConfig {
    Oscillator(Box<OscillatorConfig>),
    Envelope(Box<EnvelopeConfig>),
    Lfo(Box<LfoConfig>),
    Amplifier(Box<AmplifierConfig>),
    Mixer(Box<MixerConfig>),
    WaveShaper(Box<WaveShaperConfig>),
    SpectralFilter(Box<SpectralFilterConfig>),
    SpectralBlend(Box<SpectralBlendConfig>),
    SpectralMixer(Box<SpectralMixerConfig>),
    HarmonicEditor(Box<HarmonicEditorConfig>),
    Expressions(Box<ExpressionsConfig>),
    ExternalParam(Box<ExternalParamConfig>),
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct FullConfig {
    pub engine: EngineConfig,
    pub modules: Vec<ModuleConfig>,
    pub links: Vec<LinkConfig>,
}

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

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Config {
    pub routing: RoutingConfig,
}
