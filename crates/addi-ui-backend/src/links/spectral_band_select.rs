use triple_buffer::triple_buffer;

use addi_engine::spectral_band_select::BandSelectMode;
use addi_engine::{DISPLAY_SPECTRUM_SIZE, Sample, UI_TO_AUDIO_RING_CAPACITY, types::ComplexSample};

use addi_engine::spectral_band_select::{
    SpectralBandSelectAudioEnd, SpectralBandSelectLinks, SpectralBandSelectUiEnd, UiEvent,
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

    pub fn set_mode(&mut self, mode: BandSelectMode) -> bool {
        self.tx.push(UiEvent::Mode(mode)).is_ok()
    }

    pub fn set_harmonic_from(&mut self, value: u16) -> bool {
        self.tx.push(UiEvent::HarmonicFrom(value)).is_ok()
    }

    pub fn set_harmonic_to(&mut self, value: u16) -> bool {
        self.tx.push(UiEvent::HarmonicTo(value)).is_ok()
    }

    pub fn set_freq_from(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::FreqFrom(value)).is_ok()
    }

    pub fn set_freq_to(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::FreqTo(value)).is_ok()
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

impl SpectralBandSelectAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_spectrum(&mut self, spectrum: &[ComplexSample]) {
        AudioEnd::update_spectrum(self, spectrum)
    }
}

impl SpectralBandSelectUiEnd for UiEnd {
    fn get_spectrum(&mut self) -> &[ComplexSample] {
        UiEnd::get_spectrum(self)
    }
    fn set_mode(&mut self, mode: BandSelectMode) -> bool {
        UiEnd::set_mode(self, mode)
    }
    fn set_harmonic_from(&mut self, value: u16) -> bool {
        UiEnd::set_harmonic_from(self, value)
    }
    fn set_harmonic_to(&mut self, value: u16) -> bool {
        UiEnd::set_harmonic_to(self, value)
    }
    fn set_freq_from(&mut self, value: Sample) -> bool {
        UiEnd::set_freq_from(self, value)
    }
    fn set_freq_to(&mut self, value: Sample) -> bool {
        UiEnd::set_freq_to(self, value)
    }
}

pub struct Links;

impl SpectralBandSelectLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
