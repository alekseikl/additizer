use crate::synth_engine::{Input, Smp, StereoSample, engine_io};
use addi_dsp::filters::svf::SvfType;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    FilterType(SvfType),
    Keytrack(Smp),
}

pub trait SvfAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_pitch(&mut self, pitch: Smp);
}

pub trait SvfUiEnd: Send {
    fn pitch(&mut self) -> Smp;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_filter_type(&mut self, filter_type: SvfType) -> bool;
    fn set_keytrack(&mut self, value: Smp) -> bool;
}

pub trait SvfLinks: Send {
    type AudioEnd: SvfAudioEnd;
    type UiEnd: SvfUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
