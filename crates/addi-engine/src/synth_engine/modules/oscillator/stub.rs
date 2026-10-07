use super::{
    OscillatorAudioEnd, OscillatorLinks, OscillatorUiEnd, PhasesDst, UiEvent, Unison, UnisonStyle,
};

use crate::synth_engine::{ComplexSample, DisplaySpectrum, Input, Sample, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl OscillatorLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl OscillatorAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn publish_unison(&mut self, _unison: &Unison) {}

    fn update_spectrum(&mut self, _spectrum: &[ComplexSample]) {}
}

impl OscillatorUiEnd for NoUi {
    fn get_spectrum(&mut self) -> &DisplaySpectrum {
        self.diverge()
    }

    fn get_unison_mut(&mut self) -> &mut Unison {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_unison(&mut self, _unison: usize) -> bool {
        self.diverge()
    }

    fn set_unison_style(&mut self, _style: UnisonStyle) -> bool {
        self.diverge()
    }

    fn set_unison_stereo(&mut self, _stereo: Sample) -> bool {
        self.diverge()
    }

    fn set_steal_phase(&mut self, _steal_phase: bool) -> bool {
        self.diverge()
    }

    fn set_phase_random(&mut self, _phase_random: Sample) -> bool {
        self.diverge()
    }

    fn set_phase_random_stereo(&mut self, _phase_random_stereo: bool) -> bool {
        self.diverge()
    }

    fn set_mono_spectrum(&mut self, _mono_spectrum: bool) -> bool {
        self.diverge()
    }

    fn set_unison_initial_phase(&mut self, _idx: usize, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_unison_phase_shift(&mut self, _idx: usize, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_unison_phase_shift_to(&mut self, _idx: usize, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_unison_gain(&mut self, _idx: usize, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_unison_gain_to(&mut self, _idx: usize, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn apply_unison_level_shape(
        &mut self,
        _center: StereoSample,
        _level: StereoSample,
        _to: bool,
    ) -> bool {
        self.diverge()
    }

    fn randomize_phases(
        &mut self,
        _amount: Sample,
        _stereo_spread: Sample,
        _dst: PhasesDst,
    ) -> bool {
        self.diverge()
    }
}
