use super::{PitchAudioEnd, PitchLinks, PitchUiEnd, UiEvent};

use crate::synth_engine::{Input, Sample, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl PitchLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl PitchAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_pitch(&mut self, _pitch: Sample) {}
}

impl PitchUiEnd for NoUi {
    fn get_pitch(&mut self) -> Sample {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_keytrack(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_glide_always(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_glide_per_octave(&mut self, _value: bool) -> bool {
        self.diverge()
    }
}
