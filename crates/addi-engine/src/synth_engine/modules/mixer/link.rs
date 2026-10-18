use crate::synth_engine::{Input, LevelType, Smp, StereoSample, engine_io};

pub enum UiEvent {
    InputParam {
        input: Input,
        value: StereoSample,
    },
    NumInputs(u8),
    InputVolumeType {
        input_idx: u8,
        volume_type: LevelType,
    },
    OutputVolumeType(LevelType),
}

pub enum UiUpdate {
    RefreshRouting,
}

pub trait MixerAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn refresh_routing(&mut self) -> bool;
    fn update_out_volume(&mut self, channel_idx: usize, out_volume: Smp);
}

pub trait MixerUiEnd: Send {
    fn get_out_volume(&mut self) -> StereoSample;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_num_inputs(&mut self, num_inputs: u8) -> bool;
    fn set_volume_type(&mut self, input_idx: u8, volume_type: LevelType) -> bool;
    fn set_output_volume_type(&mut self, volume_type: LevelType) -> bool;
    fn pop_update(&mut self) -> Option<UiUpdate>;
}

pub trait MixerLinks: Send {
    type AudioEnd: MixerAudioEnd;
    type UiEnd: MixerUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
