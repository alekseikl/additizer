
use crate::synth_engine::{
    Input, StereoSample, types::ComplexSample,
};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
}

pub trait SpectralBlendAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_spectrum(&mut self, spectrum: &[ComplexSample]);
}

pub trait SpectralBlendUiEnd: Send {
    fn get_spectrum(&mut self) -> &[ComplexSample];
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
}

pub trait SpectralBlendLinks: Send {
    type AudioEnd: SpectralBlendAudioEnd;
    type UiEnd: SpectralBlendUiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
