use addi_dsp::segments::Segments;

use crate::synth_engine::{Input, Smp, StereoSample, engine_io};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    SegmentsUpdated,
    Bipolar(bool),
    Smooth(Smp),
}

pub trait ModMapAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_probe(&mut self, probe: StereoSample);
    fn read_segments(&mut self) -> &Segments;
}

pub trait ModMapUiEnd: Send {
    fn probe(&mut self) -> StereoSample;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_segments(&mut self, segments: Segments) -> bool;
    fn set_bipolar(&mut self, value: bool) -> bool;
    fn set_smooth(&mut self, value: Smp) -> bool;
}

pub trait ModMapLinks: Send {
    type AudioEnd: ModMapAudioEnd;
    type UiEnd: ModMapUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
