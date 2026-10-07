use triple_buffer::triple_buffer;

use addi_engine::{
    DISPLAY_SPECTRUM_SIZE, DisplaySpectrum, Input, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY,
    buffer::copy_to_display_spectrum, oscillator::PhasesDst, types::ComplexSample,
};

use addi_engine::oscillator::{
    OscillatorAudioEnd, OscillatorLinks, OscillatorUiEnd, UiEvent, Unison,
};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    spectrum: triple_buffer::Output<DisplaySpectrum>,
    unison: triple_buffer::Output<Unison>,
}

impl UiEnd {
    pub fn get_spectrum(&mut self) -> &DisplaySpectrum {
        self.spectrum.update();
        self.spectrum.output_buffer()
    }

    pub fn get_unison_mut(&mut self) -> &mut Unison {
        self.unison.update();
        self.unison.output_buffer_mut()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_unison(&mut self, unison: usize) -> bool {
        self.tx.push(UiEvent::Unison(unison)).is_ok()
    }

    pub fn set_steal_phase(&mut self, steal_phase: bool) -> bool {
        self.tx.push(UiEvent::StealPhase(steal_phase)).is_ok()
    }

    pub fn set_phase_random(&mut self, phase_random: Sample) -> bool {
        self.tx.push(UiEvent::PhaseRandom(phase_random)).is_ok()
    }

    pub fn set_phase_random_stereo(&mut self, phase_random_stereo: bool) -> bool {
        self.tx
            .push(UiEvent::PhaseRandomStereo(phase_random_stereo))
            .is_ok()
    }

    pub fn set_mono_spectrum(&mut self, mono_spectrum: bool) -> bool {
        self.tx.push(UiEvent::MonoSpectrum(mono_spectrum)).is_ok()
    }

    pub fn set_unison_initial_phase(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx
            .push(UiEvent::UnisonInitialPhase { idx, value })
            .is_ok()
    }

    pub fn set_unison_phase_shift(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx
            .push(UiEvent::UnisonPhaseShift { idx, value })
            .is_ok()
    }

    pub fn set_unison_phase_shift_to(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx
            .push(UiEvent::UnisonPhaseShiftTo { idx, value })
            .is_ok()
    }

    pub fn set_unison_gain(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx.push(UiEvent::UnisonGain { idx, value }).is_ok()
    }

    pub fn set_unison_gain_to(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx.push(UiEvent::UnisonGainTo { idx, value }).is_ok()
    }

    pub fn apply_unison_level_shape(
        &mut self,
        center: StereoSample,
        level: StereoSample,
        to: bool,
    ) -> bool {
        self.tx
            .push(UiEvent::ApplyUnisonLevelShape { center, level, to })
            .is_ok()
    }

    pub fn randomize_phases(
        &mut self,
        amount: Sample,
        stereo_spread: Sample,
        dst: PhasesDst,
    ) -> bool {
        self.tx
            .push(UiEvent::RandomizePhases {
                amount,
                stereo_spread,
                dst,
            })
            .is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    spectrum: triple_buffer::Input<DisplaySpectrum>,
    unison: triple_buffer::Input<Unison>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn publish_unison(&mut self, unison: &Unison) {
        *self.unison.input_buffer_mut() = *unison;
        self.unison.publish();
    }

    pub fn update_spectrum(&mut self, spectrum: &[ComplexSample]) {
        copy_to_display_spectrum(self.spectrum.input_buffer_mut(), spectrum);
        self.spectrum.publish();
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (spectrum_input, spectrum_output) =
        triple_buffer(&[ComplexSample::ZERO; DISPLAY_SPECTRUM_SIZE]);
    let (unison_input, unison_output) = triple_buffer(&Unison::default());

    (
        AudioEnd {
            rx: from_ui_rx,
            spectrum: spectrum_input,
            unison: unison_input,
        },
        UiEnd {
            tx: to_audio_tx,
            spectrum: spectrum_output,
            unison: unison_output,
        },
    )
}

impl OscillatorAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn publish_unison(&mut self, unison: &Unison) {
        AudioEnd::publish_unison(self, unison)
    }
    fn update_spectrum(&mut self, spectrum: &[ComplexSample]) {
        AudioEnd::update_spectrum(self, spectrum)
    }
}

impl OscillatorUiEnd for UiEnd {
    fn get_spectrum(&mut self) -> &DisplaySpectrum {
        UiEnd::get_spectrum(self)
    }
    fn get_unison_mut(&mut self) -> &mut Unison {
        UiEnd::get_unison_mut(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_unison(&mut self, unison: usize) -> bool {
        UiEnd::set_unison(self, unison)
    }
    fn set_steal_phase(&mut self, steal_phase: bool) -> bool {
        UiEnd::set_steal_phase(self, steal_phase)
    }
    fn set_phase_random(&mut self, phase_random: Sample) -> bool {
        UiEnd::set_phase_random(self, phase_random)
    }
    fn set_phase_random_stereo(&mut self, phase_random_stereo: bool) -> bool {
        UiEnd::set_phase_random_stereo(self, phase_random_stereo)
    }
    fn set_mono_spectrum(&mut self, mono_spectrum: bool) -> bool {
        UiEnd::set_mono_spectrum(self, mono_spectrum)
    }
    fn set_unison_initial_phase(&mut self, idx: usize, value: StereoSample) -> bool {
        UiEnd::set_unison_initial_phase(self, idx, value)
    }
    fn set_unison_phase_shift(&mut self, idx: usize, value: StereoSample) -> bool {
        UiEnd::set_unison_phase_shift(self, idx, value)
    }
    fn set_unison_phase_shift_to(&mut self, idx: usize, value: StereoSample) -> bool {
        UiEnd::set_unison_phase_shift_to(self, idx, value)
    }
    fn set_unison_gain(&mut self, idx: usize, value: StereoSample) -> bool {
        UiEnd::set_unison_gain(self, idx, value)
    }
    fn set_unison_gain_to(&mut self, idx: usize, value: StereoSample) -> bool {
        UiEnd::set_unison_gain_to(self, idx, value)
    }
    fn apply_unison_level_shape(
        &mut self,
        center: StereoSample,
        level: StereoSample,
        to: bool,
    ) -> bool {
        UiEnd::apply_unison_level_shape(self, center, level, to)
    }
    fn randomize_phases(&mut self, amount: Sample, stereo_spread: Sample, dst: PhasesDst) -> bool {
        UiEnd::randomize_phases(self, amount, stereo_spread, dst)
    }
}

pub struct Links;

impl OscillatorLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
