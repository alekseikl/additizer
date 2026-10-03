use crate::synth_engine::{Input, Sample, StereoSample, engine_io};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
}

pub trait AmplifierAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_out_volume(&mut self, channel_idx: usize, out_volume: Sample);
}

pub trait AmplifierUiEnd: Send {
    fn get_out_volume(&mut self) -> StereoSample;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
}

pub trait AmplifierLinks: Send {
    type AudioEnd: AmplifierAudioEnd;
    type UiEnd: AmplifierUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
