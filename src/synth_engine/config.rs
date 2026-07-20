use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        Input, MAX_BLOCK_SIZE, SPECTRAL_BUFFER_SIZE, Sample, StereoSample,
        amplifier::AmplifierConfig, envelope::EnvelopeConfig, expressions::ExpressionsConfig,
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
    pub bandwidth: usize,
}

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            num_voices: 1,
            legato: false,
            block_size: MAX_BLOCK_SIZE,
            oversampling: false,
            stereo_spectrum: true,
            voice_kill_time: from_ms(30.0),
            output_gain: 0.5.into(),
            bandwidth: 0,
        }
    }
}

pub const MAX_BANDWIDTH: usize = SPECTRAL_BUFFER_SIZE - 1;

#[derive(Clone, Serialize, Deserialize)]
pub enum LinkConfig {
    Direct {
        src_id: ModuleId,
        dst_id: ModuleId,
        dst_input: Input,
    },
    Mixed {
        src_id: ModuleId,
        dst_id: ModuleId,
        dst_input: Input,
        amount: StereoSample,
        modulator_id: Option<ModuleId>,
    },
}

impl LinkConfig {
    pub fn direct(src_id: ModuleId, dst_id: ModuleId, dst_input: Input) -> Self {
        Self::Direct {
            src_id,
            dst_id,
            dst_input,
        }
    }

    pub fn mixed(
        src_id: ModuleId,
        dst_id: ModuleId,
        dst_input: Input,
        amount: impl Into<StereoSample>,
    ) -> Self {
        Self::Mixed {
            src_id,
            dst_id,
            dst_input,
            amount: amount.into(),
            modulator_id: None,
        }
    }

    pub fn mixed_modulated(
        src_id: ModuleId,
        dst_id: ModuleId,
        dst_input: Input,
        amount: impl Into<StereoSample>,
        modulator_id: Option<ModuleId>,
    ) -> Self {
        Self::Mixed {
            src_id,
            dst_id,
            dst_input,
            amount: amount.into(),
            modulator_id,
        }
    }

    pub fn is_direct(&self) -> bool {
        matches!(self, Self::Direct { .. })
    }

    pub fn src_id(&self) -> ModuleId {
        match self {
            Self::Direct { src_id, .. } | Self::Mixed { src_id, .. } => *src_id,
        }
    }

    pub fn dst_id(&self) -> ModuleId {
        match self {
            Self::Direct { dst_id, .. } | Self::Mixed { dst_id, .. } => *dst_id,
        }
    }

    pub fn dst_input(&self) -> Input {
        match self {
            Self::Direct { dst_input, .. } | Self::Mixed { dst_input, .. } => *dst_input,
        }
    }

    pub fn amount(&self) -> StereoSample {
        match self {
            Self::Direct { .. } => StereoSample::ONE,
            Self::Mixed { amount, .. } => *amount,
        }
    }

    pub fn modulator_id(&self) -> Option<ModuleId> {
        match self {
            Self::Direct { .. } => None,
            Self::Mixed { modulator_id, .. } => *modulator_id,
        }
    }

    pub fn set_modulator_id(&mut self, id: Option<ModuleId>) -> bool {
        match self {
            Self::Mixed { modulator_id, .. } => {
                *modulator_id = id;
                true
            }
            Self::Direct { .. } => false,
        }
    }
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
