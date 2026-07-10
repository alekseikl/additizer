use triple_buffer::triple_buffer;

use crate::synth_engine::{Input, StereoSample, types::ComplexSample};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
}

pub const DISPLAY_SPECTRUM_SIZE: usize = 256;
pub type DisplaySpectrum = [ComplexSample; DISPLAY_SPECTRUM_SIZE];

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
        let input_buff = self.spectrum.input_buffer_mut();
        let len = spectrum.len().min(DISPLAY_SPECTRUM_SIZE);

        input_buff[..len].copy_from_slice(&spectrum[..len]);
        input_buff[len..].fill(ComplexSample::ZERO);
        self.spectrum.publish();
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(128);
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
