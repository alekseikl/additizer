use serde::{Deserialize, Serialize};

use crate::synth_engine::{Sample, StereoSample, buffer::SpectralBuffer};

use super::buffer::Buffer;

pub type ModuleId = i64;

pub const MAX_VOICES: usize = 24;
pub const NUM_CHANNELS: usize = 2;
pub const OUTPUT_MODULE_ID: ModuleId = 0;
pub const MIN_MODULE_ID: ModuleId = 1;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum ModuleType {
    Envelope,
    Amplifier,
    Mixer,
    Oscillator,
    SpectralFilter,
    SpectralBlend,
    SpectralMixer,
    HarmonicEditor,
    ExternalParam,
    ModulationFilter,
    Lfo,
    WaveShaper,
    One,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DataType {
    Buffer,
    Scalar,
    Spectral,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum Input {
    Audio,
    AudioMix(usize),
    Gain, // 0.0 - 1.0
    GainMix(usize),
    Level,           // dB
    LevelMix(usize), // dB
    Distortion,      // dB
    ClippingLevel,   // dB
    PitchShift,
    Detune,
    PhaseShift,
    FrequencyShift,
    Spectrum,
    SpectrumMix(usize),
    SpectrumTo,
    Blend,
    LowFrequency,
    Cutoff,
    Q,
    Drive, // dB
    Skew,
    Delay,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
}

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VolumeType {
    #[default]
    Gain,
    Db,
}

impl VolumeType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Db => "dB",
            Self::Gain => "Gain",
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MixType {
    #[default]
    Add,
    Subtract,
    Multiply,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub struct ModuleInput {
    pub input_type: Input,
    pub module_id: ModuleId,
}

impl ModuleInput {
    pub fn new(input: Input, id: ModuleId) -> Self {
        Self {
            input_type: input,
            module_id: id,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LinkModulation {
    pub src: ModuleId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModuleLink {
    pub src: ModuleId,
    pub dst: ModuleInput,
    pub amount: StereoSample,
    pub modulation: Option<LinkModulation>,
}

impl ModuleLink {
    pub fn link(src: ModuleId, dst: ModuleInput) -> Self {
        Self {
            src,
            dst,
            amount: StereoSample::ONE,
            modulation: None,
        }
    }

    pub fn modulation(src: ModuleId, dst: ModuleInput, amount: impl Into<StereoSample>) -> Self {
        Self {
            src,
            dst,
            amount: amount.into(),
            modulation: None,
        }
    }
}

pub struct AvailableInputSourceUI {
    pub src: ModuleId,
    pub label: String,
}

pub struct InputModulationUI {
    #[allow(unused)]
    pub src: ModuleId,
    pub label: String,
}

pub struct ConnectedInputSourceUI {
    pub src: ModuleId,
    pub amount: StereoSample,
    pub label: String,
    pub modulation: Option<InputModulationUI>,
}

pub trait Router {
    fn get_input<'a>(
        &'a self,
        input: ModuleInput,
        samples: usize,
        voice_idx: usize,
        channel_idx: usize,
        input_buffer: &'a mut Buffer,
    ) -> Option<&'a Buffer>;

    fn read_unmodulated_input(
        &self,
        input: ModuleInput,
        samples: usize,
        voice_idx: usize,
        channel_idx: usize,
        input_buffer: &mut Buffer,
    ) -> bool;

    fn get_spectral_input(
        &self,
        input: ModuleInput,
        current: bool,
        voice_idx: usize,
        channel_idx: usize,
    ) -> Option<&SpectralBuffer>;

    fn get_scalar_input(
        &self,
        input: ModuleInput,
        current: bool,
        voice_idx: usize,
        channel_idx: usize,
    ) -> Option<Sample>;
}
