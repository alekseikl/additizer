use crate::synth_engine::{Input, StereoSample, engine_io};

use super::config::ShaperType;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    ShaperType(ShaperType),
}

pub trait WaveShaperAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
}

pub trait WaveShaperUiEnd: Send {
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_shaper_type(&mut self, shaper_type: ShaperType) -> bool;
}

pub trait WaveShaperLinks: Send {
    type AudioEnd: WaveShaperAudioEnd;
    type UiEnd: WaveShaperUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
