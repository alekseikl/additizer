use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        Input, MAX_BLOCK_SIZE, Sample, StereoSample, amplifier::AmplifierConfig,
        envelope::EnvelopeConfig, expressions::ExpressionsConfig,
        external_param::ExternalParamConfig, harmonic_editor::HarmonicEditorConfig, lfo::LfoConfig,
        mixer::MixerConfig, oscillator::OscillatorConfig, routing::ModuleId,
        spectral_blend::SpectralBlendConfig, spectral_filter::SpectralFilterConfig,
        spectral_mixer::SpectralMixerConfig, wave_shaper::WaveShaperConfig,
    },
    utils::from_ms,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct EngineParams {
    pub num_voices: usize,
    pub legato: bool,
    pub block_size: usize,
    pub oversampling: bool,
    pub stereo_spectrum: bool,
    pub voice_kill_time: Sample,
    pub output_gain: StereoSample,
}

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            num_voices: 1,
            legato: false,
            block_size: MAX_BLOCK_SIZE,
            oversampling: false,
            stereo_spectrum: true,
            voice_kill_time: from_ms(50.0),
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
pub struct EngineConfig {
    pub engine: EngineParams,
    pub modules: Vec<ModuleConfig>,
    pub links: Vec<LinkConfig>,
}
