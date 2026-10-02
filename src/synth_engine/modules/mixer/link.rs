use crate::synth_engine::{Input, Sample, StereoSample, VolumeType, engine_io};

pub enum UiEvent {
    InputParam {
        input: Input,
        value: StereoSample,
    },
    NumInputs(u8),
    InputVolumeType {
        input_idx: u8,
        volume_type: VolumeType,
    },
    OutputVolumeType(VolumeType),
}

pub enum UiUpdate {
    RefreshRouting,
}

pub trait MixerAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn refresh_routing(&mut self) -> bool;
    fn update_out_volume(&mut self, channel_idx: usize, out_volume: Sample);
}

pub trait MixerUiEnd: Send {
    fn get_out_volume(&mut self) -> StereoSample;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_num_inputs(&mut self, num_inputs: u8) -> bool;
    fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) -> bool;
    fn set_output_volume_type(&mut self, volume_type: VolumeType) -> bool;
    fn pop_update(&mut self) -> Option<UiUpdate>;
}

pub trait MixerLinks: Send {
    type AudioEnd: MixerAudioEnd;
    type UiEnd: MixerUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
