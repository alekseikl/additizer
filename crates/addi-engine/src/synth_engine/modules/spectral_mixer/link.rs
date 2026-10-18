use crate::synth_engine::{
    Input, LevelType, MixType, StereoSample, engine_io, types::ComplexSample,
};

pub enum UiEvent {
    InputParam {
        input: Input,
        value: StereoSample,
    },
    NumInputs(u8),
    MixType {
        input_idx: u8,
        mix_type: MixType,
    },
    LevelType {
        input_idx: u8,
        volume_type: LevelType,
    },
    OutputVolumeType(LevelType),
}

pub enum UiUpdate {
    RefreshRouting,
}

pub trait SpectralMixerAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn refresh_routing(&mut self) -> bool;
    fn update_spectrum(&mut self, spectrum: &[ComplexSample]);
}

pub trait SpectralMixerUiEnd: Send {
    fn get_spectrum(&mut self) -> &[ComplexSample];
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_num_inputs(&mut self, num_inputs: u8) -> bool;
    fn set_mix_type(&mut self, input_idx: u8, mix_type: MixType) -> bool;
    fn set_volume_type(&mut self, input_idx: u8, volume_type: LevelType) -> bool;
    fn set_output_volume_type(&mut self, volume_type: LevelType) -> bool;
    fn pop_update(&mut self) -> Option<UiUpdate>;
}

pub trait SpectralMixerLinks: Send {
    type AudioEnd: SpectralMixerAudioEnd;
    type UiEnd: SpectralMixerUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
