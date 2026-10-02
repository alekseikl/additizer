use triple_buffer::triple_buffer;

use crate::synth_engine::{
    AUDIO_TO_UI_RING_CAPACITY, DISPLAY_SPECTRUM_SIZE, Input, MixType, StereoSample,
    UI_TO_AUDIO_RING_CAPACITY, VolumeType, types::ComplexSample,
};

use crate::synth_engine::spectral_mixer::{SpectralMixerAudioEnd, SpectralMixerLinks, SpectralMixerUiEnd, UiEvent, UiUpdate};

pub struct UiEnd {
    rx: rtrb::Consumer<UiUpdate>,
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

    pub fn set_num_inputs(&mut self, num_inputs: u8) -> bool {
        self.tx.push(UiEvent::NumInputs(num_inputs)).is_ok()
    }

    pub fn set_mix_type(&mut self, input_idx: u8, mix_type: MixType) -> bool {
        self.tx
            .push(UiEvent::MixType {
                input_idx,
                mix_type,
            })
            .is_ok()
    }

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) -> bool {
        self.tx
            .push(UiEvent::VolumeType {
                input_idx,
                volume_type,
            })
            .is_ok()
    }

    pub fn set_output_volume_type(&mut self, volume_type: VolumeType) -> bool {
        self.tx.push(UiEvent::OutputVolumeType(volume_type)).is_ok()
    }

    pub fn pop_update(&mut self) -> Option<UiUpdate> {
        self.rx.pop().ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    tx: rtrb::Producer<UiUpdate>,
    spectrum: triple_buffer::Input<Vec<ComplexSample>>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn refresh_routing(&mut self) -> bool {
        self.tx.push(UiUpdate::RefreshRouting).is_ok()
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
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(AUDIO_TO_UI_RING_CAPACITY);
    // Length, not only capacity: `triple_buffer` clones this, and `Vec::clone` keeps `len`.
    let spectrum = vec![ComplexSample::ZERO; DISPLAY_SPECTRUM_SIZE];
    let (spectrum_input, spectrum_output) = triple_buffer(&spectrum);

    (
        AudioEnd {
            rx: from_ui_rx,
            tx: to_ui_tx,
            spectrum: spectrum_input,
        },
        UiEnd {
            rx: from_audio_rx,
            tx: to_audio_tx,
            spectrum: spectrum_output,
        },
    )
}

impl SpectralMixerAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn refresh_routing(&mut self) -> bool {
        AudioEnd::refresh_routing(self)
    }
    fn update_spectrum(&mut self, spectrum: &[ComplexSample]) {
        AudioEnd::update_spectrum(self, spectrum)
    }
}

impl SpectralMixerUiEnd for UiEnd {
    fn get_spectrum(&mut self) -> &[ComplexSample] {
        UiEnd::get_spectrum(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_num_inputs(&mut self, num_inputs: u8) -> bool {
        UiEnd::set_num_inputs(self, num_inputs)
    }
    fn set_mix_type(&mut self, input_idx: u8, mix_type: MixType) -> bool {
        UiEnd::set_mix_type(self, input_idx, mix_type)
    }
    fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) -> bool {
        UiEnd::set_volume_type(self, input_idx, volume_type)
    }
    fn set_output_volume_type(&mut self, volume_type: VolumeType) -> bool {
        UiEnd::set_output_volume_type(self, volume_type)
    }
    fn pop_update(&mut self) -> Option<UiUpdate> {
        UiEnd::pop_update(self)
    }
}

pub struct Links;

impl SpectralMixerLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd) {
        make_link_pair()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_length_follows_the_source() {
        let (mut audio, mut ui) = make_link_pair();
        let long = vec![ComplexSample::from_polar(0.25, 0.1); DISPLAY_SPECTRUM_SIZE + 8];

        audio.update_spectrum(&long);
        assert_eq!(ui.get_spectrum().len(), DISPLAY_SPECTRUM_SIZE);
        assert_eq!(ui.get_spectrum()[3], long[3]);

        let short = vec![ComplexSample::from_polar(0.5, -0.2); 6];
        audio.update_spectrum(&short);
        assert_eq!(ui.get_spectrum(), short.as_slice());
        assert!(audio.spectrum.input_buffer().capacity() >= DISPLAY_SPECTRUM_SIZE);
    }
}
