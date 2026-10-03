use triple_buffer::triple_buffer;

use addi_engine::{
    DISPLAY_SPECTRUM_SIZE, Input, StereoSample, UI_TO_AUDIO_RING_CAPACITY, types::ComplexSample,
};

use addi_engine::spectral_blend::{
    SpectralBlendAudioEnd, SpectralBlendLinks, SpectralBlendUiEnd, UiEvent,
};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    spectrum: triple_buffer::Output<Vec<ComplexSample>>,
}

impl UiEnd {
    pub fn get_spectrum(&mut self) -> &[ComplexSample] {
        self.spectrum.update();
        self.spectrum.output_buffer()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    spectrum: triple_buffer::Input<Vec<ComplexSample>>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_spectrum(&mut self, spectrum: &[ComplexSample]) {
        let dst = self.spectrum.input_buffer_mut();
        let len = spectrum.len().min(DISPLAY_SPECTRUM_SIZE);

        debug_assert!(dst.capacity() >= len);
        dst.clear();
        dst.extend_from_slice(&spectrum[..len]);
        self.spectrum.publish();
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    // Length, not only capacity: `triple_buffer` clones this, and `Vec::clone` keeps `len`.
    let spectrum = vec![ComplexSample::ZERO; DISPLAY_SPECTRUM_SIZE];
    let (spectrum_input, spectrum_output) = triple_buffer(&spectrum);

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

impl SpectralBlendAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_spectrum(&mut self, spectrum: &[ComplexSample]) {
        AudioEnd::update_spectrum(self, spectrum)
    }
}

impl SpectralBlendUiEnd for UiEnd {
    fn get_spectrum(&mut self) -> &[ComplexSample] {
        UiEnd::get_spectrum(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
}

pub struct Links;

impl SpectralBlendLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}

#[cfg(test)]
mod tests;
