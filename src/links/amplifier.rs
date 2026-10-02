use triple_buffer::triple_buffer;

use crate::synth_engine::{Input, NUM_CHANNELS, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY};

use crate::synth_engine::amplifier::{AmplifierAudioEnd, AmplifierLinks, AmplifierUiEnd, UiEvent};

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

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
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

impl AmplifierAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_out_volume(&mut self, channel_idx: usize, out_volume: Sample) {
        AudioEnd::update_out_volume(self, channel_idx, out_volume)
    }
}

impl AmplifierUiEnd for UiEnd {
    fn get_out_volume(&mut self) -> StereoSample {
        UiEnd::get_out_volume(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
}

pub struct Links;

impl AmplifierLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
