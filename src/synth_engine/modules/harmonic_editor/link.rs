use std::array;

use triple_buffer::triple_buffer;

use crate::synth_engine::{
    AUDIO_TO_UI_RING_CAPACITY, ComplexSample, DISPLAY_SPECTRUM_SIZE, DisplaySpectrum, NUM_CHANNELS,
    SPECTRAL_BUFFER_SIZE, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY,
    buffer::copy_to_display_spectrum,
};

use super::SetParams;

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

pub enum UiEvent {
    SetHarmonic {
        harmonic_number: usize,
        gain: StereoSample,
    },
    SetSelected(SetParams),
}

pub enum UiUpdate {
    RefreshState,
}

pub struct UiEnd {
    rx: rtrb::Consumer<UiUpdate>,
    tx: rtrb::Producer<UiEvent>,
    display_spectrum: triple_buffer::Output<Box<DisplaySpectrum>>,
    harmonics: triple_buffer::Output<Harmonics>,
}

impl UiEnd {
    pub fn get_display_spectrum(&mut self) -> &DisplaySpectrum {
        self.display_spectrum.update();
        self.display_spectrum.output_buffer()
    }

    pub fn get_harmonics(&mut self) -> &Harmonics {
        self.harmonics.update();
        self.harmonics.output_buffer()
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) -> bool {
        self.tx
            .push(UiEvent::SetHarmonic {
                harmonic_number,
                gain,
            })
            .is_ok()
    }

    pub fn set_selected(&mut self, params: SetParams) -> bool {
        self.tx.push(UiEvent::SetSelected(params)).is_ok()
    }

    pub fn pop_update(&mut self) -> Option<UiUpdate> {
        self.rx.pop().ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    tx: rtrb::Producer<UiUpdate>,
    display_spectrum: triple_buffer::Input<Box<DisplaySpectrum>>,
    harmonics: triple_buffer::Input<Harmonics>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn push_refresh_state(&mut self) -> bool {
        self.tx.push(UiUpdate::RefreshState).is_ok()
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
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(AUDIO_TO_UI_RING_CAPACITY);
    let (display_spectrum_input, display_spectrum_output) =
        triple_buffer(&Box::new([ComplexSample::ZERO; DISPLAY_SPECTRUM_SIZE]));
    let (harmonics_input, harmonics_output) = triple_buffer(&Harmonics::default());

    (
        AudioEnd {
            rx: from_ui_rx,
            tx: to_ui_tx,
            display_spectrum: display_spectrum_input,
            harmonics: harmonics_input,
        },
        UiEnd {
            rx: from_audio_rx,
            tx: to_audio_tx,
            display_spectrum: display_spectrum_output,
            harmonics: harmonics_output,
        },
    )
}
