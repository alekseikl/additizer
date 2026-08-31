use std::array;

use triple_buffer::triple_buffer;

use crate::synth_engine::{
    ComplexSample, DISPLAY_SPECTRUM_SIZE, DisplaySpectrum, NUM_CHANNELS, SPECTRAL_BUFFER_SIZE,
    Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY, buffer::copy_to_display_spectrum,
    harmonic_editor::EditRequest,
};

#[derive(Clone)]
pub struct Harmonics {
    amplitudes: [Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
    phases: [Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
}

impl Default for Harmonics {
    fn default() -> Self {
        Self {
            amplitudes: array::from_fn(|_| Box::new([0.0; SPECTRAL_BUFFER_SIZE])),
            phases: array::from_fn(|_| Box::new([0.0; SPECTRAL_BUFFER_SIZE])),
        }
    }
}

impl Harmonics {
    pub fn amplitude(&self, index: usize) -> StereoSample {
        StereoSample::new(self.amplitudes[0][index], self.amplitudes[1][index])
    }

    pub fn set_amplitude(&mut self, index: usize, gain: StereoSample) {
        for (amplitudes, &gain) in self.amplitudes.iter_mut().zip(gain.iter()) {
            amplitudes[index] = gain;
        }
    }

    pub fn phase(&self, index: usize) -> StereoSample {
        StereoSample::new(self.phases[0][index], self.phases[1][index])
    }

    pub fn set_phase(&mut self, index: usize, phase: StereoSample) {
        for (phases, &phase) in self.phases.iter_mut().zip(phase.iter()) {
            phases[index] = phase;
        }
    }
}

pub enum UiEvent {
    SetAmplitude { index: u32, gain: StereoSample },
    SetPhase { index: u32, phase: StereoSample },
    Clear,
    ResetSawtooth,
    EditRequest(EditRequest),
    ApplyDraft,
    DiscardDraft,
}

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

    pub fn edit_request(&mut self, request: EditRequest) -> bool {
        self.tx.push(UiEvent::EditRequest(request)).is_ok()
    }

    pub fn apply_draft(&mut self) -> bool {
        self.tx.push(UiEvent::ApplyDraft).is_ok()
    }

    pub fn discard_draft(&mut self) -> bool {
        self.tx.push(UiEvent::DiscardDraft).is_ok()
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

        for (dst, src) in dst.amplitudes.iter_mut().zip(amplitudes.iter()) {
            dst.copy_from_slice(src.as_slice());
        }

        for (dst, src) in dst.phases.iter_mut().zip(phases.iter()) {
            dst.copy_from_slice(src.as_slice());
        }

        self.harmonics.publish();
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
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
