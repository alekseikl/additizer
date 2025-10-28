use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    synth_module::{ScalarOutputs, SpectralOutputs},
    types::StereoSample,
};

use super::buffer::Buffer;

pub const MAX_VOICES: usize = 16;
pub const NUM_CHANNELS: usize = 2;
pub type ModuleId = u64;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LinkDataType {
    Buffer,
    Scalar,
    Spectral,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum RoutingNode {
    Envelope(ModuleId),
    Amplifier(ModuleId),
    Oscillator(ModuleId),
    SpectralFilter(ModuleId),
    Output,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum ModuleInput {
    AmplifierInput(ModuleId),
    AmplifierLevel(ModuleId),
    OscillatorSpectrum(ModuleId),
    OscillatorLevel(ModuleId),
    OscillatorPitchShift(ModuleId),
    OscillatorDetune(ModuleId),
    SpectralFilterCutoff(ModuleId),
    Output,
}

impl ModuleInput {
    pub fn routing_node(&self) -> RoutingNode {
        match self {
            Self::AmplifierInput(module_id) => RoutingNode::Amplifier(*module_id),
            Self::AmplifierLevel(module_id) => RoutingNode::Amplifier(*module_id),
            Self::OscillatorLevel(module_id) => RoutingNode::Oscillator(*module_id),
            Self::OscillatorSpectrum(id) => RoutingNode::Oscillator(*id),
            Self::OscillatorPitchShift(module_id) => RoutingNode::Oscillator(*module_id),
            Self::OscillatorDetune(id) => RoutingNode::Oscillator(*id),
            Self::SpectralFilterCutoff(id) => RoutingNode::SpectralFilter(*id),
            Self::Output => RoutingNode::Output,
        }
    }

    pub fn data_type(&self) -> LinkDataType {
        match self {
            Self::AmplifierInput(_) => LinkDataType::Buffer,
            Self::AmplifierLevel(_) => LinkDataType::Buffer,
            Self::OscillatorLevel(_) => LinkDataType::Buffer,
            Self::OscillatorSpectrum(_) => LinkDataType::Spectral,
            Self::OscillatorPitchShift(_) => LinkDataType::Buffer,
            Self::OscillatorDetune(_) => LinkDataType::Buffer,
            Self::SpectralFilterCutoff(_) => LinkDataType::Scalar,
            Self::Output => LinkDataType::Buffer,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum ModuleOutput {
    Envelope(ModuleId),
    EnvelopeScalar(ModuleId),
    Amplifier(ModuleId),
    Oscillator(ModuleId),
    SpectralFilter(ModuleId),
}

impl ModuleOutput {
    pub fn routing_node(&self) -> RoutingNode {
        match self {
            Self::Envelope(module_id) => RoutingNode::Envelope(*module_id),
            Self::EnvelopeScalar(module_id) => RoutingNode::Envelope(*module_id),
            Self::Amplifier(module_id) => RoutingNode::Amplifier(*module_id),
            Self::Oscillator(module_id) => RoutingNode::Oscillator(*module_id),
            Self::SpectralFilter(id) => RoutingNode::SpectralFilter(*id),
        }
    }

    pub fn data_type(&self) -> LinkDataType {
        match self {
            Self::Envelope(_) => LinkDataType::Buffer,
            Self::EnvelopeScalar(_) => LinkDataType::Scalar,
            Self::Amplifier(_) => LinkDataType::Buffer,
            Self::Oscillator(_) => LinkDataType::Buffer,
            Self::SpectralFilter(_) => LinkDataType::Spectral,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleInputSource {
    pub src: ModuleOutput,
    pub modulation: Option<StereoSample>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModuleLink {
    pub src: ModuleOutput,
    pub dst: ModuleInput,
    pub modulation: Option<StereoSample>,
}

impl ModuleLink {
    pub fn link(src: ModuleOutput, dst: ModuleInput) -> Self {
        Self {
            src,
            dst,
            modulation: None,
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
            modulation: Some(amount.into()),
        }
    }
}

pub trait Router {
    fn get_input<'a>(
        &'a self,
        input: ModuleInput,
        voice_idx: usize,
        channel: usize,
        input_buffer: &'a mut Buffer,
    ) -> Option<&'a Buffer>;

    fn get_spectral_input(
        &self,
        input: ModuleInput,
        voice_idx: usize,
        channel: usize,
    ) -> Option<SpectralOutputs<'_>>;

    fn get_scalar_input(
        &self,
        input: ModuleInput,
        voice_idx: usize,
        channel: usize,
    ) -> Option<ScalarOutputs>;
}
