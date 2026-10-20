use addi_dsp::segments::Segments;

use crate::synth_engine::{Input, Smp, StereoSample, engine_io};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    SegmentsUpdated,
    StealPhase(bool),
}

pub trait LfoAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_phase(&mut self, phase: Smp);
    fn read_segments(&mut self) -> &Segments;
}

pub trait LfoUiEnd: Send {
    fn get_phase(&mut self) -> Smp;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_segments(&mut self, segments: Segments) -> bool;
    fn set_steal_phase(&mut self, value: bool) -> bool;
}

pub trait LfoLinks: Send {
    type AudioEnd: LfoAudioEnd;
    type UiEnd: LfoUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
