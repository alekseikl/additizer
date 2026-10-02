use super::{
    BandSelectMode, SpectralBandSelectAudioEnd, SpectralBandSelectLinks, SpectralBandSelectUiEnd,
    UiEvent,
};

use crate::synth_engine::{ComplexSample, Sample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl SpectralBandSelectLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl SpectralBandSelectAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_spectrum(&mut self, _spectrum: &[ComplexSample]) {}
}

impl SpectralBandSelectUiEnd for NoUi {
    fn get_spectrum(&mut self) -> &[ComplexSample] {
        self.diverge()
    }

    fn set_mode(&mut self, _mode: BandSelectMode) -> bool {
        self.diverge()
    }

    fn set_harmonic_from(&mut self, _value: u16) -> bool {
        self.diverge()
    }

    fn set_harmonic_to(&mut self, _value: u16) -> bool {
        self.diverge()
    }

    fn set_freq_from(&mut self, _value: Sample) -> bool {
        self.diverge()
    }

    fn set_freq_to(&mut self, _value: Sample) -> bool {
        self.diverge()
    }
}
