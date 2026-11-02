use addi_dsp::time_segments::TimeSegments;

use super::MultiEnvelopePhase;
use crate::synth_engine::engine_io;

pub enum UiEvent {
    SegmentsUpdated,
    StealLevel(bool),
    KeepVoiceAlive(bool),
}

pub enum UiUpdate {
    RefreshRouting,
}

pub trait MultiEnvelopeAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_phase(&mut self, phase: MultiEnvelopePhase);
    fn read_segments(&mut self) -> &TimeSegments;
    fn refresh_routing(&mut self) -> bool;
}

pub trait MultiEnvelopeUiEnd: Send {
    fn get_phase(&mut self) -> MultiEnvelopePhase;
    fn set_segments(&mut self, segments: TimeSegments) -> bool;
    fn set_steal_level(&mut self, value: bool) -> bool;
    fn set_keep_voice_alive(&mut self, value: bool) -> bool;
    fn pop_update(&mut self) -> Option<UiUpdate>;
}

pub trait MultiEnvelopeLinks: Send {
    type AudioEnd: MultiEnvelopeAudioEnd;
    type UiEnd: MultiEnvelopeUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
