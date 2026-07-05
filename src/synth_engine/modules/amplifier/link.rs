use triple_buffer::triple_buffer;

use crate::synth_engine::{Input, NUM_CHANNELS, Sample, StereoSample};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
}

pub struct UiEnd {
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
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    out_volume: triple_buffer::Input<StereoSample>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_out_volume(&mut self, channel_idx: usize, out_volume: Sample) {
        self.out_volume.input_buffer_mut()[channel_idx] = out_volume;

        if channel_idx == NUM_CHANNELS - 1 {
            self.out_volume.publish();
        }
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(128);
    let (out_volume_input, out_volume_output) = triple_buffer(&StereoSample::ZERO);

    (
        AudioEnd {
            rx: from_ui_rx,
            out_volume: out_volume_input,
        },
        UiEnd {
            tx: to_audio_tx,
            out_volume: out_volume_output,
        },
    )
}
