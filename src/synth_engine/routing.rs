use serde::{Deserialize, Serialize};

use crate::synth_engine::{Sample, StereoSample, buffer::SpectralBuffer};

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
    Lfo,
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
    PhaseShiftScalar,
    LowFrequency,
    Cutoff,
    Q,
    Skew,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
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

    input_ctor!(audio, Audio);
    input_ctor!(scalar_input, ScalarInput);
    input_ctor!(level, Level);
    input_ctor!(pitch_shift, PitchShift);
    input_ctor!(detune, Detune);
    input_ctor!(phase_shift, PhaseShift);
    input_ctor!(spectrum, Spectrum);
    input_ctor!(phase_shift_scalar, PhaseShiftScalar);
    input_ctor!(low_frequency, LowFrequency);
    input_ctor!(cutoff, Cutoff);
    input_ctor!(q, Q);
    input_ctor!(skew, Skew);
    input_ctor!(attack, Attack);
    input_ctor!(hold, Hold);
    input_ctor!(decay, Decay);
    input_ctor!(sustain, Sustain);
    input_ctor!(release, Release);
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModuleLink {
    pub src: ModuleId,
    pub dst: ModuleInput,
    pub modulation: StereoSample,
}

impl ModuleLink {
    pub fn link(src: ModuleId, dst: ModuleInput) -> Self {
        Self {
            src,
            dst,
            modulation: StereoSample::ONE,
        }
    }

    pub fn modulation(src: ModuleId, dst: ModuleInput, amount: impl Into<StereoSample>) -> Self {
        Self {
            src,
            dst,
            modulation: amount.into(),
        }
    }
}

pub struct AvailableInputSourceUI {
    pub output: ModuleId,
    pub label: String,
}

pub struct ConnectedInputSourceUI {
    pub output: ModuleId,
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
