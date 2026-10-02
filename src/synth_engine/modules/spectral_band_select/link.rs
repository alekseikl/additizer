use super::BandSelectMode;
use crate::synth_engine::{Sample, engine_io, types::ComplexSample};

pub enum UiEvent {
    Mode(BandSelectMode),
    HarmonicFrom(u16),
    HarmonicTo(u16),
    FreqFrom(Sample),
    FreqTo(Sample),
}

pub trait SpectralBandSelectAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_spectrum(&mut self, spectrum: &[ComplexSample]);
}

pub trait SpectralBandSelectUiEnd: Send {
    fn get_spectrum(&mut self) -> &[ComplexSample];
    fn set_mode(&mut self, mode: BandSelectMode) -> bool;
    fn set_harmonic_from(&mut self, value: u16) -> bool;
    fn set_harmonic_to(&mut self, value: u16) -> bool;
    fn set_freq_from(&mut self, value: Sample) -> bool;
    fn set_freq_to(&mut self, value: Sample) -> bool;
}

pub trait SpectralBandSelectLinks: Send {
    type AudioEnd: SpectralBandSelectAudioEnd;
    type UiEnd: SpectralBandSelectUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
