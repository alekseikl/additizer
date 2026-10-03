use super::{EqFilter, SpectralEqAudioEnd, SpectralEqLinks, SpectralEqUiEnd, UiEvent};

use crate::synth_engine::{Input, Sample, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl SpectralEqLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl SpectralEqAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_pitch(&mut self, _pitch: Sample) {}
}

impl SpectralEqUiEnd for NoUi {
    fn pitch(&mut self) -> Sample {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_linear_phase(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_keytrack(&mut self, _value: Sample) -> bool {
        self.diverge()
    }

    fn set_q_cutoff(&mut self, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_q_rolloff(&mut self, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_filter(&mut self, _index: u8, _filter: EqFilter) -> bool {
        self.diverge()
    }

    fn add_filter(&mut self, _filter: EqFilter) -> bool {
        self.diverge()
    }

    fn remove_filter(&mut self, _index: u8) -> bool {
        self.diverge()
    }

    fn move_filter(&mut self, _from: u8, _to: u8) -> bool {
        self.diverge()
    }
}
