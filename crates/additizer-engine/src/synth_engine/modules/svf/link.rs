use crate::synth_engine::{Input, Sample, StereoSample, engine_io};
use additizer_dsp::filters::svf::SvfType;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    FilterType(SvfType),
    Keytrack(Sample),
}

pub trait SvfAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_pitch(&mut self, pitch: Sample);
}

pub trait SvfUiEnd: Send {
    fn pitch(&mut self) -> Sample;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_filter_type(&mut self, filter_type: SvfType) -> bool;
    fn set_keytrack(&mut self, value: Sample) -> bool;
}

pub trait SvfLinks: Send {
    type AudioEnd: SvfAudioEnd;
    type UiEnd: SvfUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
