use addi_dsp::filters::svf::SvfType;

use super::{SvfAudioEnd, SvfLinks, SvfUiEnd, UiEvent};

use crate::synth_engine::{Input, Sample, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl SvfLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl SvfAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_pitch(&mut self, _pitch: Sample) {}
}

impl SvfUiEnd for NoUi {
    fn pitch(&mut self) -> Sample {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_filter_type(&mut self, _filter_type: SvfType) -> bool {
        self.diverge()
    }

    fn set_keytrack(&mut self, _value: Sample) -> bool {
        self.diverge()
    }
}
