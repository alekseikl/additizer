use super::{AmplifierAudioEnd, AmplifierLinks, AmplifierUiEnd, UiEvent};

use crate::synth_engine::{Input, Sample, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl AmplifierLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl AmplifierAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_out_volume(&mut self, _channel_idx: usize, _out_volume: Sample) {}
}

impl AmplifierUiEnd for NoUi {
    fn get_out_volume(&mut self) -> StereoSample {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }
}
