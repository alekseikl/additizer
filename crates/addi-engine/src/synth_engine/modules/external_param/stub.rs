use super::{ExternalParamAudioEnd, ExternalParamLinks, ExternalParamUiEnd, UiEvent};

use crate::synth_engine::Smp;

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl ExternalParamLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl ExternalParamAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_value(&mut self, _value: Smp) {}
}

impl ExternalParamUiEnd for NoUi {
    fn get_value(&mut self) -> Smp {
        self.diverge()
    }

    fn select_param(&mut self, _index: usize) -> bool {
        self.diverge()
    }

    fn set_sample_on_trigger(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_polyphonic(&mut self, _value: bool) -> bool {
        self.diverge()
    }
}
