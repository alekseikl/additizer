use addi_dsp::segments::Segments;

use super::{SegmentsEnvelopePhase, Timebase};
use crate::synth_engine::{Input, StereoSample, engine_io};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    SegmentsUpdated,
    StealLevel(bool),
    KeepVoiceAlive(bool),
    Timebase(Timebase),
}

pub trait SegmentsEnvelopeAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_phase(&mut self, phase: SegmentsEnvelopePhase);
    fn read_segments(&mut self) -> &Segments;
}

pub trait SegmentsEnvelopeUiEnd: Send {
    fn get_phase(&mut self) -> SegmentsEnvelopePhase;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_segments(&mut self, segments: Segments) -> bool;
    fn set_steal_level(&mut self, value: bool) -> bool;
    fn set_keep_voice_alive(&mut self, value: bool) -> bool;
    fn set_timebase(&mut self, timebase: Timebase) -> bool;
}

pub trait SegmentsEnvelopeLinks: Send {
    type AudioEnd: SegmentsEnvelopeAudioEnd;
    type UiEnd: SegmentsEnvelopeUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
