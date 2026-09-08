use triple_buffer::triple_buffer;

use crate::synth_engine::{
    Input, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY, filters::spectral_filter::FilterType,
};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    FilterType(FilterType),
    LinearPhase(bool),
    Keytrack(Sample),
    QLimitTo(StereoSample),
    QLimitSlope(StereoSample),
}

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    pitch: triple_buffer::Output<Sample>,
}

impl UiEnd {
    pub fn pitch(&mut self) -> Sample {
        *self.pitch.read()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_filter_type(&mut self, filter_type: FilterType) -> bool {
        self.tx.push(UiEvent::FilterType(filter_type)).is_ok()
    }

    pub fn set_linear_phase(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::LinearPhase(value)).is_ok()
    }

    pub fn set_keytrack(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::Keytrack(value)).is_ok()
    }

    pub fn set_q_limit_to(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::QLimitTo(value)).is_ok()
    }

    pub fn set_q_limit_slope(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::QLimitSlope(value)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    pitch: triple_buffer::Input<Sample>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_pitch(&mut self, pitch: Sample) {
        self.pitch.write(pitch);
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (pitch_input, pitch_output) = triple_buffer(&crate::utils::C4_PITCH);

    (
        AudioEnd {
            rx: from_ui_rx,
            pitch: pitch_input,
        },
        UiEnd {
            tx: to_audio_tx,
            pitch: pitch_output,
        },
    )
}
