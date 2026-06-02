use crate::synth_engine::{Input, MixType, StereoSample, VolumeType};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    NumInputs(u8),
    MixType { input_idx: u8, mix_type: MixType },
    VolumeType { input_idx: u8, volume_type: VolumeType },
    OutputVolumeType(VolumeType),
}

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
}

impl UiEnd {
    pub fn new(tx: rtrb::Producer<UiEvent>) -> Self {
        Self { tx }
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
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
}

impl AudioEnd {
    pub fn new(rx: rtrb::Consumer<UiEvent>) -> Self {
        Self { rx }
    }

    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(256);

    (AudioEnd::new(from_ui_rx), UiEnd::new(to_audio_tx))
}
