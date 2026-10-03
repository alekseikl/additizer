use super::{ShaperType, UiEvent, WaveShaperAudioEnd, WaveShaperLinks, WaveShaperUiEnd};

use crate::synth_engine::{Input, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl WaveShaperLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl WaveShaperAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }
}

impl WaveShaperUiEnd for NoUi {
    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_shaper_type(&mut self, _shaper_type: ShaperType) -> bool {
        self.diverge()
    }
}
