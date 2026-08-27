use triple_buffer::triple_buffer;

use crate::synth_engine::{
    AUDIO_TO_UI_RING_CAPACITY, DISPLAY_SPECTRUM_SIZE, DisplaySpectrum, Input, MixType,
    StereoSample, UI_TO_AUDIO_RING_CAPACITY, VolumeType, buffer::copy_to_display_spectrum,
    types::ComplexSample,
};

pub enum UiEvent {
    InputParam {
        input: Input,
        value: StereoSample,
    },
    NumInputs(u8),
    MixType {
        input_idx: u8,
        mix_type: MixType,
    },
    VolumeType {
        input_idx: u8,
        volume_type: VolumeType,
    },
    OutputVolumeType(VolumeType),
}

pub enum UiUpdate {
    RefreshRouting,
}

pub struct UiEnd {
    rx: rtrb::Consumer<UiUpdate>,
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
    spectrum: triple_buffer::Input<Box<DisplaySpectrum>>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn refresh_routing(&mut self) -> bool {
        self.tx.push(UiUpdate::RefreshRouting).is_ok()
    }

    pub fn update_spectrum(&mut self, spectrum: &[ComplexSample]) {
        copy_to_display_spectrum(self.spectrum.input_buffer_mut(), spectrum);
        self.spectrum.publish();
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(AUDIO_TO_UI_RING_CAPACITY);
    let (spectrum_input, spectrum_output) =
        triple_buffer(&Box::new([ComplexSample::ZERO; DISPLAY_SPECTRUM_SIZE]));

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
