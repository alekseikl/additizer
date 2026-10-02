use triple_buffer::triple_buffer;

use crate::synth_engine::{
    ComplexSample, DISPLAY_SPECTRUM_SIZE, DisplaySpectrum, NUM_CHANNELS, SPECTRAL_BUFFER_SIZE,
    Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY, buffer::copy_to_display_spectrum,
    harmonic_editor::EditRequest,
};

use crate::synth_engine::harmonic_editor::{HarmonicEditorAudioEnd, HarmonicEditorLinks, HarmonicEditorUiEnd, Harmonics, UiEvent};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    display_spectrum: triple_buffer::Output<DisplaySpectrum>,
    harmonics: triple_buffer::Output<Harmonics>,
}

impl UiEnd {
    pub fn get_display_spectrum(&mut self) -> &DisplaySpectrum {
        self.display_spectrum.update();
        self.display_spectrum.output_buffer()
    }

    pub fn get_harmonics_mut(&mut self) -> &mut Harmonics {
        self.harmonics.update();
        self.harmonics.output_buffer_mut()
    }

    pub fn set_amplitude(&mut self, index: usize, gain: StereoSample) -> bool {
        self.tx
            .push(UiEvent::SetAmplitude {
                index: index as u32,
                gain,
            })
            .is_ok()
    }

    pub fn set_phase(&mut self, index: usize, phase: StereoSample) -> bool {
        self.tx
            .push(UiEvent::SetPhase {
                index: index as u32,
                phase,
            })
            .is_ok()
    }

    pub fn clear(&mut self) -> bool {
        self.tx.push(UiEvent::Clear).is_ok()
    }

    pub fn reset_sawtooth(&mut self) -> bool {
        self.tx.push(UiEvent::ResetSawtooth).is_ok()
    }

    pub fn zero_phases(&mut self) -> bool {
        self.tx.push(UiEvent::ZeroPhases).is_ok()
    }

    pub fn sawtooth_phases(&mut self) -> bool {
        self.tx.push(UiEvent::SawtoothPhases).is_ok()
    }

    pub fn random_phases(&mut self) -> bool {
        self.tx.push(UiEvent::RandomPhases).is_ok()
    }

    pub fn random_phases_stereo(&mut self) -> bool {
        self.tx.push(UiEvent::RandomPhasesStereo).is_ok()
    }

    pub fn edit_request(&mut self, request: EditRequest) -> bool {
        self.tx.push(UiEvent::EditRequest(request)).is_ok()
    }

    pub fn apply_draft(&mut self) -> bool {
        self.tx.push(UiEvent::ApplyDraft).is_ok()
    }

    pub fn discard_draft(&mut self) -> bool {
        self.tx.push(UiEvent::DiscardDraft).is_ok()
    }

    pub fn set_bandwidth(&mut self, bandwidth: i32) -> bool {
        self.tx.push(UiEvent::Bandwidth(bandwidth)).is_ok()
    }

    pub fn set_mono(&mut self, mono: bool) -> bool {
        self.tx.push(UiEvent::Mono(mono)).is_ok()
    }

    pub fn set_capture_input(&mut self, capture_input: bool) -> bool {
        self.tx.push(UiEvent::CaptureInput(capture_input)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    display_spectrum: triple_buffer::Input<DisplaySpectrum>,
    harmonics: triple_buffer::Input<Harmonics>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_display_spectrum(&mut self, spectrum: &[ComplexSample]) {
        copy_to_display_spectrum(self.display_spectrum.input_buffer_mut(), spectrum);
        self.display_spectrum.publish();
    }

    pub fn publish_harmonics(
        &mut self,
        amplitudes: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
        phases: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
    ) {
        let dst = self.harmonics.input_buffer_mut();
        dst.copy_channels(amplitudes, phases);
        self.harmonics.publish();
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (display_spectrum_input, display_spectrum_output) =
        triple_buffer(&[ComplexSample::ZERO; DISPLAY_SPECTRUM_SIZE]);
    let (harmonics_input, harmonics_output) = triple_buffer(&Harmonics::default());

    (
        AudioEnd {
            rx: from_ui_rx,
            display_spectrum: display_spectrum_input,
            harmonics: harmonics_input,
        },
        UiEnd {
            tx: to_audio_tx,
            display_spectrum: display_spectrum_output,
            harmonics: harmonics_output,
        },
    )
}

impl HarmonicEditorAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_display_spectrum(&mut self, spectrum: &[ComplexSample]) {
        AudioEnd::update_display_spectrum(self, spectrum)
    }
    fn publish_harmonics(&mut self, amplitudes: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS], phases: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS]) {
        AudioEnd::publish_harmonics(self, amplitudes, phases)
    }
}

impl HarmonicEditorUiEnd for UiEnd {
    fn get_display_spectrum(&mut self) -> &DisplaySpectrum {
        UiEnd::get_display_spectrum(self)
    }
    fn get_harmonics_mut(&mut self) -> &mut Harmonics {
        UiEnd::get_harmonics_mut(self)
    }
    fn set_amplitude(&mut self, index: usize, gain: StereoSample) -> bool {
        UiEnd::set_amplitude(self, index, gain)
    }
    fn set_phase(&mut self, index: usize, phase: StereoSample) -> bool {
        UiEnd::set_phase(self, index, phase)
    }
    fn clear(&mut self) -> bool {
        UiEnd::clear(self)
    }
    fn reset_sawtooth(&mut self) -> bool {
        UiEnd::reset_sawtooth(self)
    }
    fn zero_phases(&mut self) -> bool {
        UiEnd::zero_phases(self)
    }
    fn sawtooth_phases(&mut self) -> bool {
        UiEnd::sawtooth_phases(self)
    }
    fn random_phases(&mut self) -> bool {
        UiEnd::random_phases(self)
    }
    fn random_phases_stereo(&mut self) -> bool {
        UiEnd::random_phases_stereo(self)
    }
    fn edit_request(&mut self, request: EditRequest) -> bool {
        UiEnd::edit_request(self, request)
    }
    fn apply_draft(&mut self) -> bool {
        UiEnd::apply_draft(self)
    }
    fn discard_draft(&mut self) -> bool {
        UiEnd::discard_draft(self)
    }
    fn set_bandwidth(&mut self, bandwidth: i32) -> bool {
        UiEnd::set_bandwidth(self, bandwidth)
    }
    fn set_mono(&mut self, mono: bool) -> bool {
        UiEnd::set_mono(self, mono)
    }
    fn set_capture_input(&mut self, capture_input: bool) -> bool {
        UiEnd::set_capture_input(self, capture_input)
    }
}

pub struct Links;

impl HarmonicEditorLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd) {
        make_link_pair()
    }
}
