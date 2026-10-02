use super::{MixerAudioEnd, MixerLinks, MixerUiEnd, UiEvent, UiUpdate};

use crate::synth_engine::{Input, Sample, StereoSample, VolumeType};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl MixerLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl MixerAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn refresh_routing(&mut self) -> bool {
        false
    }

    fn update_out_volume(&mut self, _channel_idx: usize, _out_volume: Sample) {}
}

impl MixerUiEnd for NoUi {
    fn get_out_volume(&mut self) -> StereoSample {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_num_inputs(&mut self, _num_inputs: u8) -> bool {
        self.diverge()
    }

    fn set_volume_type(&mut self, _input_idx: u8, _volume_type: VolumeType) -> bool {
        self.diverge()
    }

    fn set_output_volume_type(&mut self, _volume_type: VolumeType) -> bool {
        self.diverge()
    }

    fn pop_update(&mut self) -> Option<UiUpdate> {
        self.diverge()
    }
}
