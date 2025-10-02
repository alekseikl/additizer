use crate::buffer::Buffer;

pub const MAX_VOICES: usize = 8;
pub type ModuleId = u64;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum RoutingNode {
    Envelope(ModuleId),
    Amplifier(ModuleId),
    Oscillator(ModuleId),
    Output,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ModuleInput {
    AmplifierInput(ModuleId),
    AmplifierLevel(ModuleId),
    OscillatorLevel(ModuleId),
    OscillatorPitchShift(ModuleId),
    Output,
}

impl ModuleInput {
    pub fn routing_node(&self) -> RoutingNode {
        match self {
            Self::AmplifierInput(module_id) => RoutingNode::Amplifier(*module_id),
            Self::AmplifierLevel(module_id) => RoutingNode::Amplifier(*module_id),
            Self::OscillatorLevel(module_id) => RoutingNode::Oscillator(*module_id),
            Self::OscillatorPitchShift(module_id) => RoutingNode::Oscillator(*module_id),
            Self::Output => RoutingNode::Output,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ModuleOutput {
    Envelope(ModuleId),
    Amplifier(ModuleId),
    Oscillator(ModuleId),
}

impl ModuleOutput {
    pub fn routing_node(&self) -> RoutingNode {
        match self {
            Self::Envelope(module_id) => RoutingNode::Envelope(*module_id),
            Self::Amplifier(module_id) => RoutingNode::Amplifier(*module_id),
            Self::Oscillator(module_id) => RoutingNode::Oscillator(*module_id),
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
        input_buffer: &'a mut Buffer,
    ) -> Option<&'a Buffer>;
}
