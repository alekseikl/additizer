use crate::synth_engine::{Input, Sample, StereoSample, engine_io};

use super::EnvelopePhase;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    AttackSlope(Sample),
    DecaySlope(Sample),
    ReleaseSlope(Sample),
    KeepVoiceAlive(bool),
    StealLevel(bool),
}

pub trait EnvelopeAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_phase(&mut self, phase: EnvelopePhase);
}

pub trait EnvelopeUiEnd: Send {
    fn get_phase(&mut self) -> EnvelopePhase;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_attack_slope(&mut self, value: Sample) -> bool;
    fn set_decay_slope(&mut self, value: Sample) -> bool;
    fn set_release_slope(&mut self, value: Sample) -> bool;
    fn set_keep_voice_alive(&mut self, value: bool) -> bool;
    fn set_steal_level(&mut self, value: bool) -> bool;
}

pub trait EnvelopeLinks: Send {
    type AudioEnd: EnvelopeAudioEnd;
    type UiEnd: EnvelopeUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
