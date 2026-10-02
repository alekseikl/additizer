use triple_buffer::triple_buffer;

use crate::synth_engine::{
    ComplexSample, DISPLAY_SPECTRUM_SIZE, DisplaySpectrum, Input, StereoSample,
    UI_TO_AUDIO_RING_CAPACITY, buffer::copy_to_display_spectrum,
};

use crate::synth_engine::spectral_noise::NoiseColor;

use crate::synth_engine::spectral_noise::{SpectralNoiseAudioEnd, SpectralNoiseLinks, SpectralNoiseUiEnd, UiEvent};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    display_spectrum: triple_buffer::Output<DisplaySpectrum>,
}

impl UiEnd {
    pub fn get_display_spectrum(&mut self) -> &DisplaySpectrum {
        self.display_spectrum.update();
        self.display_spectrum.output_buffer()
    }

    pub fn set_color(&mut self, color: NoiseColor) -> bool {
        self.tx.push(UiEvent::Color(color)).is_ok()
    }

    pub fn set_bandwidth(&mut self, bandwidth: i32) -> bool {
        self.tx.push(UiEvent::Bandwidth(bandwidth)).is_ok()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_stereo(&mut self, stereo: bool) -> bool {
        self.tx.push(UiEvent::Stereo(stereo)).is_ok()
    }

    pub fn set_cutoff(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::Cutoff(value)).is_ok()
    }

    pub fn set_rolloff(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::Rolloff(value)).is_ok()
    }

    pub fn set_steal_phase(&mut self, steal_phase: bool) -> bool {
        self.tx.push(UiEvent::StealPhase(steal_phase)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    display_spectrum: triple_buffer::Input<DisplaySpectrum>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_display_spectrum(&mut self, spectrum: &[ComplexSample]) {
        copy_to_display_spectrum(self.display_spectrum.input_buffer_mut(), spectrum);
        self.display_spectrum.publish();
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (display_spectrum_input, display_spectrum_output) =
        triple_buffer(&[ComplexSample::ZERO; DISPLAY_SPECTRUM_SIZE]);

    (
        AudioEnd {
            rx: from_ui_rx,
            display_spectrum: display_spectrum_input,
        },
        UiEnd {
            tx: to_audio_tx,
            display_spectrum: display_spectrum_output,
        },
    )
}

impl SpectralNoiseAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_display_spectrum(&mut self, spectrum: &[ComplexSample]) {
        AudioEnd::update_display_spectrum(self, spectrum)
    }
}

impl SpectralNoiseUiEnd for UiEnd {
    fn get_display_spectrum(&mut self) -> &DisplaySpectrum {
        UiEnd::get_display_spectrum(self)
    }
    fn set_color(&mut self, color: NoiseColor) -> bool {
        UiEnd::set_color(self, color)
    }
    fn set_bandwidth(&mut self, bandwidth: i32) -> bool {
        UiEnd::set_bandwidth(self, bandwidth)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_stereo(&mut self, stereo: bool) -> bool {
        UiEnd::set_stereo(self, stereo)
    }
    fn set_cutoff(&mut self, value: StereoSample) -> bool {
        UiEnd::set_cutoff(self, value)
    }
    fn set_rolloff(&mut self, value: StereoSample) -> bool {
        UiEnd::set_rolloff(self, value)
    }
    fn set_steal_phase(&mut self, steal_phase: bool) -> bool {
        UiEnd::set_steal_phase(self, steal_phase)
    }
}

pub struct Links;

impl SpectralNoiseLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd) {
        make_link_pair()
    }
}
