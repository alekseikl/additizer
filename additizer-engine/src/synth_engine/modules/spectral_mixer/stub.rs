use super::{SpectralMixerAudioEnd, SpectralMixerLinks, SpectralMixerUiEnd, UiEvent, UiUpdate};

use crate::synth_engine::{ComplexSample, Input, MixType, StereoSample, VolumeType};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl SpectralMixerLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl SpectralMixerAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn refresh_routing(&mut self) -> bool {
        false
    }

    fn update_spectrum(&mut self, _spectrum: &[ComplexSample]) {}
}

impl SpectralMixerUiEnd for NoUi {
    fn get_spectrum(&mut self) -> &[ComplexSample] {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_num_inputs(&mut self, _num_inputs: u8) -> bool {
        self.diverge()
    }

    fn set_mix_type(&mut self, _input_idx: u8, _mix_type: MixType) -> bool {
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
