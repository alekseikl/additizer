use super::{SpectralBlendAudioEnd, SpectralBlendLinks, SpectralBlendUiEnd, UiEvent};

use crate::synth_engine::{ComplexSample, Input, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl SpectralBlendLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl SpectralBlendAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_spectrum(&mut self, _spectrum: &[ComplexSample]) {}
}

impl SpectralBlendUiEnd for NoUi {
    fn get_spectrum(&mut self) -> &[ComplexSample] {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }
}
