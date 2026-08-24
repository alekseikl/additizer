use triple_buffer::triple_buffer;

use crate::synth_engine::{
    AUDIO_TO_UI_RING_CAPACITY, Input, NUM_CHANNELS, Sample, StereoSample,
    UI_TO_AUDIO_RING_CAPACITY, VolumeType,
};

pub enum UiEvent {
    InputParam {
        input: Input,
        value: StereoSample,
    },
    NumInputs(u8),
    InputVolumeType {
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
    out_volume: triple_buffer::Output<StereoSample>,
}

impl UiEnd {
    pub fn get_out_volume(&mut self) -> StereoSample {
        *self.out_volume.read()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_num_inputs(&mut self, num_inputs: u8) -> bool {
        self.tx.push(UiEvent::NumInputs(num_inputs)).is_ok()
    }

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) -> bool {
        self.tx
            .push(UiEvent::InputVolumeType {
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
    out_volume: triple_buffer::Input<StereoSample>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn refresh_routing(&mut self) -> bool {
        self.tx.push(UiUpdate::RefreshRouting).is_ok()
    }

    pub fn update_out_volume(&mut self, channel_idx: usize, out_volume: Sample) {
        self.out_volume.input_buffer_mut()[channel_idx] = out_volume;

        if channel_idx == NUM_CHANNELS - 1 {
            self.out_volume.publish();
        }
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(AUDIO_TO_UI_RING_CAPACITY);
    let (out_volume_input, out_volume_output) = triple_buffer(&StereoSample::ZERO);

    (
        AudioEnd {
            rx: from_ui_rx,
            tx: to_ui_tx,
            out_volume: out_volume_input,
        },
        UiEnd {
            rx: from_audio_rx,
            tx: to_audio_tx,
            out_volume: out_volume_output,
        },
    )
}
