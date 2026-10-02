use super::{NoiseColor, SpectralNoiseAudioEnd, SpectralNoiseLinks, SpectralNoiseUiEnd, UiEvent};

use crate::synth_engine::{ComplexSample, DisplaySpectrum, Input, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl SpectralNoiseLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl SpectralNoiseAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_display_spectrum(&mut self, _spectrum: &[ComplexSample]) {}
}

impl SpectralNoiseUiEnd for NoUi {
    fn get_display_spectrum(&mut self) -> &DisplaySpectrum {
        self.diverge()
    }

    fn set_color(&mut self, _color: NoiseColor) -> bool {
        self.diverge()
    }

    fn set_bandwidth(&mut self, _bandwidth: i32) -> bool {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_stereo(&mut self, _stereo: bool) -> bool {
        self.diverge()
    }

    fn set_cutoff(&mut self, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_rolloff(&mut self, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_steal_phase(&mut self, _steal_phase: bool) -> bool {
        self.diverge()
    }
}
