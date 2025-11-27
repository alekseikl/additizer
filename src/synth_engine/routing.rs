use serde::{Deserialize, Serialize};

use crate::synth_engine::{Sample, buffer::SpectralBuffer, types::StereoSample};

use super::buffer::Buffer;

pub type ModuleId = i64;

pub const MAX_VOICES: usize = 16;
pub const NUM_CHANNELS: usize = 2;
pub const OUTPUT_MODULE_ID: ModuleId = 0;
pub const MIN_MODULE_ID: ModuleId = 1;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum ModuleType {
    Envelope,
    Amplifier,
    Oscillator,
    SpectralFilter,
    HarmonicEditor,
    ExternalParam,
    ModulationFilter,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DataType {
    Buffer,
    Scalar,
    Spectral,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum InputType {
    Audio,
    ScalarInput,
    Level,
    PitchShift,
    Detune,
    PhaseShift,
    Spectrum,
    Cutoff,
    Q,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
}

impl InputType {
    pub fn data_type(&self) -> DataType {
        match self {
            Self::Audio => DataType::Buffer,
            Self::ScalarInput => DataType::Scalar,
            Self::Level => DataType::Buffer,
            Self::PitchShift => DataType::Buffer,
            Self::Detune => DataType::Buffer,
            Self::PhaseShift => DataType::Buffer,
            Self::Spectrum => DataType::Spectral,
            Self::Cutoff => DataType::Scalar,
            Self::Q => DataType::Scalar,
            Self::Attack => DataType::Scalar,
            Self::Hold => DataType::Scalar,
            Self::Decay => DataType::Scalar,
            Self::Sustain => DataType::Scalar,
            Self::Release => DataType::Scalar,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum OutputType {
    Audio,
    Spectrum,
    Scalar,
}

impl OutputType {
    pub fn data_type(&self) -> DataType {
        match self {
            Self::Audio => DataType::Buffer,
            Self::Spectrum => DataType::Spectral,
            Self::Scalar => DataType::Scalar,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub struct ModuleInput {
    pub input_type: InputType,
    pub module_id: ModuleId,
}

macro_rules! input_ctor {
    ($ctor_name:ident, $input_type:ident) => {
        pub fn $ctor_name(id: ModuleId) -> Self {
            Self::new(InputType::$input_type, id)
        }
    };
}

impl ModuleInput {
    pub fn new(input: InputType, id: ModuleId) -> Self {
        Self {
            input_type: input,
            module_id: id,
        }
    }

    pub fn data_type(&self) -> DataType {
        self.input_type.data_type()
    }

    input_ctor!(audio, Audio);
    input_ctor!(scalar_input, ScalarInput);
    input_ctor!(level, Level);
    input_ctor!(pitch_shift, PitchShift);
    input_ctor!(detune, Detune);
    input_ctor!(phase_shift, PhaseShift);
    input_ctor!(spectrum, Spectrum);
    input_ctor!(cutoff, Cutoff);
    input_ctor!(q, Q);
    input_ctor!(attack, Attack);
    input_ctor!(hold, Hold);
    input_ctor!(decay, Decay);
    input_ctor!(sustain, Sustain);
    input_ctor!(release, Release);
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub struct ModuleOutput {
    pub output_type: OutputType,
    pub module_id: ModuleId,
}

macro_rules! output_ctor {
    ($ctor_name:ident, $input_type:ident) => {
        pub fn $ctor_name(id: ModuleId) -> Self {
            Self::new(OutputType::$input_type, id)
        }
    };
}

impl ModuleOutput {
    pub fn new(output: OutputType, id: ModuleId) -> Self {
        Self {
            output_type: output,
            module_id: id,
        }
    }

    pub fn data_type(&self) -> DataType {
        self.output_type.data_type()
    }

    output_ctor!(audio, Audio);
    output_ctor!(spectrum, Spectrum);
    output_ctor!(scalar, Scalar);
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModuleLink {
    pub src: ModuleOutput,
    pub dst: ModuleInput,
    pub modulation: StereoSample,
}

impl ModuleLink {
    pub fn link(src: ModuleOutput, dst: ModuleInput) -> Self {
        Self {
            src,
            dst,
            modulation: StereoSample::ONE,
        }
    }

    pub fn modulation(
        src: ModuleOutput,
        dst: ModuleInput,
        amount: impl Into<StereoSample>,
    ) -> Self {
        Self {
            src,
            dst,
            modulation: amount.into(),
        }
    }
}

pub struct AvailableInputSourceUI {
    pub output: ModuleOutput,
    pub label: String,
}

pub struct ConnectedInputSourceUI {
    pub output: ModuleOutput,
    pub modulation: StereoSample,
    pub label: String,
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
