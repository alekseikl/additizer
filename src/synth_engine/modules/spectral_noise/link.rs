use crate::synth_engine::{ComplexSample, DisplaySpectrum, Input, StereoSample, engine_io};

use super::config::NoiseColor;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    Color(NoiseColor),
    Bandwidth(i32),
    Stereo(bool),
    Cutoff(StereoSample),
    Rolloff(StereoSample),
    StealPhase(bool),
}

pub trait SpectralNoiseAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_display_spectrum(&mut self, spectrum: &[ComplexSample]);
}

pub trait SpectralNoiseUiEnd: Send {
    fn get_display_spectrum(&mut self) -> &DisplaySpectrum;
    fn set_color(&mut self, color: NoiseColor) -> bool;
    fn set_bandwidth(&mut self, bandwidth: i32) -> bool;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_stereo(&mut self, stereo: bool) -> bool;
    fn set_cutoff(&mut self, value: StereoSample) -> bool;
    fn set_rolloff(&mut self, value: StereoSample) -> bool;
    fn set_steal_phase(&mut self, steal_phase: bool) -> bool;
}

pub trait SpectralNoiseLinks: Send {
    type AudioEnd: SpectralNoiseAudioEnd;
    type UiEnd: SpectralNoiseUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
