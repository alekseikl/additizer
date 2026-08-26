use triple_buffer::triple_buffer;

use crate::synth_engine::{
    DISPLAY_SPECTRUM_SIZE, DisplaySpectrum, Input, StereoSample, UI_TO_AUDIO_RING_CAPACITY,
    buffer::copy_to_display_spectrum, types::ComplexSample,
};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
}

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    spectrum: triple_buffer::Output<Box<DisplaySpectrum>>,
}

impl UiEnd {
    pub fn get_spectrum(&mut self) -> &DisplaySpectrum {
        self.spectrum.update();
        self.spectrum.output_buffer()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    spectrum: triple_buffer::Input<Box<DisplaySpectrum>>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_spectrum(&mut self, spectrum: &[ComplexSample]) {
        copy_to_display_spectrum(self.spectrum.input_buffer_mut(), spectrum);
        self.spectrum.publish();
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (spectrum_input, spectrum_output) =
        triple_buffer(&Box::new([ComplexSample::ZERO; DISPLAY_SPECTRUM_SIZE]));

    (
        AudioEnd {
            rx: from_ui_rx,
            spectrum: spectrum_input,
        },
        UiEnd {
            tx: to_audio_tx,
            spectrum: spectrum_output,
        },
    )
}
