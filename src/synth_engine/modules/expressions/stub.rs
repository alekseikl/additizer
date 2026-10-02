use super::{ExpressionsAudioEnd, ExpressionsLinks, ExpressionsUiEnd, UiEvent};

use crate::synth_engine::{Expression, Sample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl ExpressionsLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl ExpressionsAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_value(&mut self, _value: Sample) {}
}

impl ExpressionsUiEnd for NoUi {
    fn get_value(&mut self) -> Sample {
        self.diverge()
    }

    fn set_expression(&mut self, _expression: Expression) -> bool {
        self.diverge()
    }

    fn set_use_release_velocity(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_smooth(&mut self, _value: Sample) -> bool {
        self.diverge()
    }
}
