use super::{
    EditRequest, HarmonicEditorAudioEnd, HarmonicEditorLinks, HarmonicEditorUiEnd, Harmonics,
    UiEvent,
};

use crate::synth_engine::{
    ComplexSample, DisplaySpectrum, NUM_CHANNELS, SPECTRAL_BUFFER_SIZE, Sample, StereoSample,
};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl HarmonicEditorLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl HarmonicEditorAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_display_spectrum(&mut self, _spectrum: &[ComplexSample]) {}

    fn publish_harmonics(
        &mut self,
        _amplitudes: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
        _phases: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
    ) {
    }
}

impl HarmonicEditorUiEnd for NoUi {
    fn get_display_spectrum(&mut self) -> &DisplaySpectrum {
        self.diverge()
    }

    fn get_harmonics_mut(&mut self) -> &mut Harmonics {
        self.diverge()
    }

    fn set_amplitude(&mut self, _index: usize, _gain: StereoSample) -> bool {
        self.diverge()
    }

    fn set_phase(&mut self, _index: usize, _phase: StereoSample) -> bool {
        self.diverge()
    }

    fn clear(&mut self) -> bool {
        self.diverge()
    }

    fn reset_sawtooth(&mut self) -> bool {
        self.diverge()
    }

    fn zero_phases(&mut self) -> bool {
        self.diverge()
    }

    fn sawtooth_phases(&mut self) -> bool {
        self.diverge()
    }

    fn random_phases(&mut self) -> bool {
        self.diverge()
    }

    fn random_phases_stereo(&mut self) -> bool {
        self.diverge()
    }

    fn edit_request(&mut self, _request: EditRequest) -> bool {
        self.diverge()
    }

    fn apply_draft(&mut self) -> bool {
        self.diverge()
    }

    fn discard_draft(&mut self) -> bool {
        self.diverge()
    }

    fn set_bandwidth(&mut self, _bandwidth: i32) -> bool {
        self.diverge()
    }

    fn set_mono(&mut self, _mono: bool) -> bool {
        self.diverge()
    }

    fn set_capture_input(&mut self, _capture_input: bool) -> bool {
        self.diverge()
    }
}
