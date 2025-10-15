use crate::synth_engine::buffer::SpectralBuffer;

use super::buffer::Buffer;

pub const MAX_VOICES: usize = 16;
pub const NUM_CHANNELS: usize = 2;
pub type ModuleId = u64;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum RoutingNode {
    Envelope(ModuleId),
    Amplifier(ModuleId),
    Oscillator(ModuleId),
    SpectralFilter(ModuleId),
    Output,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ModuleInput {
    AmplifierInput(ModuleId),
    AmplifierLevel(ModuleId),
    OscillatorSpectrum(ModuleId),
    OscillatorLevel(ModuleId),
    OscillatorPitchShift(ModuleId),
    OscillatorDetune(ModuleId),
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
            Self::Output => RoutingNode::Output,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ModuleOutput {
    Envelope(ModuleId),
    Amplifier(ModuleId),
    Oscillator(ModuleId),
    SpectralFilter(ModuleId),
}

impl ModuleOutput {
    pub fn routing_node(&self) -> RoutingNode {
        match self {
            Self::Envelope(module_id) => RoutingNode::Envelope(*module_id),
            Self::Amplifier(module_id) => RoutingNode::Amplifier(*module_id),
            Self::Oscillator(module_id) => RoutingNode::Oscillator(*module_id),
            Self::SpectralFilter(id) => RoutingNode::SpectralFilter(*id),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleInputSource {
    pub src: ModuleOutput,
    pub modulation_amount: Option<f32>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ModuleLinkPair {
    pub src: ModuleOutput,
    pub dst: ModuleInput,
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleLink {
    pub src: ModuleOutput,
    pub dst: ModuleInput,
    pub modulation_amount: Option<f32>,
}

impl ModuleLink {
    pub fn link(src: ModuleOutput, dst: ModuleInput) -> Self {
        Self {
            src,
            dst,
            modulation_amount: None,
        }
    }

    pub fn modulation(src: ModuleOutput, dst: ModuleInput, amount: f32) -> Self {
        Self {
            src,
            dst,
            modulation_amount: Some(amount),
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
    ) -> Option<&SpectralBuffer>;
}
