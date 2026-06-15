use crate::synth_engine::{
    Amplifier, Envelope, Expressions, ExternalParam, HarmonicEditor, Input, Lfo, Mixer, ModuleId,
    Oscillator, SpectralBlend, SpectralFilter, SpectralMixer, StereoSample, VoiceEvent, WaveShaper,
    modules::Output,
    routing::{DataType, InputSlots, ProcessContext, SpectralInputSlot},
    synth_module::{ModInput, SynthModule},
    voices_handler::DecayingVoice,
};
use enum_dispatch::enum_dispatch;

#[derive(Debug, Clone, Copy)]
pub enum ModuleType {
    Output,
    Envelope,
    Amplifier,
    Mixer,
    Oscillator,
    SpectralFilter,
    SpectralBlend,
    SpectralMixer,
    HarmonicEditor,
    ExternalParam,
    Lfo,
    WaveShaper,
    Expressions,
}

#[enum_dispatch(SynthModule)]
pub enum ModuleHandle {
    Oscillator(Box<Oscillator>),
    Envelope(Box<Envelope>),
    Lfo(Box<Lfo>),
    Amplifier(Box<Amplifier>),
    WaveShaper(Box<WaveShaper>),
    Mixer(Box<Mixer>),
    SpectralFilter(Box<SpectralFilter>),
    SpectralBlend(Box<SpectralBlend>),
    SpectralMixer(Box<SpectralMixer>),
    HarmonicEditor(Box<HarmonicEditor>),
    Expressions(Box<Expressions>),
    ExternalParam(Box<ExternalParam>),
    Output(Box<Output>),
}

impl ModuleHandle {
    pub(super) fn module_type(&self) -> ModuleType {
        match self {
            Self::Output(_) => ModuleType::Output,
            Self::Oscillator(_) => ModuleType::Oscillator,
            Self::Envelope(_) => ModuleType::Envelope,
            Self::Lfo(_) => ModuleType::Lfo,
            Self::Amplifier(_) => ModuleType::Amplifier,
            Self::Mixer(_) => ModuleType::Mixer,
            Self::WaveShaper(_) => ModuleType::WaveShaper,
            Self::SpectralFilter(_) => ModuleType::SpectralFilter,
            Self::SpectralBlend(_) => ModuleType::SpectralBlend,
            Self::SpectralMixer(_) => ModuleType::SpectralMixer,
            Self::HarmonicEditor(_) => ModuleType::HarmonicEditor,
            Self::Expressions(_) => ModuleType::Expressions,
            Self::ExternalParam(_) => ModuleType::ExternalParam,
        }
    }
}
