use crate::synth_engine::{Input, MixType, StereoSample, VolumeType};

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
}

impl UiEnd {
    pub fn new(rx: rtrb::Consumer<UiUpdate>, tx: rtrb::Producer<UiEvent>) -> Self {
        Self { rx, tx }
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
}

impl AudioEnd {
    pub fn new(rx: rtrb::Consumer<UiEvent>, tx: rtrb::Producer<UiUpdate>) -> Self {
        Self { rx, tx }
    }

    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn refresh_routing(&mut self) -> bool {
        self.tx.push(UiUpdate::RefreshRouting).is_ok()
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(256);
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(128);

    (
        AudioEnd::new(from_ui_rx, to_ui_tx),
        UiEnd::new(from_audio_rx, to_audio_tx),
    )
}
