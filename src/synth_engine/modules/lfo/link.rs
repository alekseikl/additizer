use triple_buffer::triple_buffer;

use crate::synth_engine::{Input, Sample, StereoSample};

use super::config::LfoShape;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    Shape(LfoShape),
    Bipolar(bool),
    StealPhase(bool),
    SmoothTime(StereoSample),
}

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    phase: triple_buffer::Output<Sample>,
}

impl UiEnd {
    pub fn get_phase(&mut self) -> Sample {
        *self.phase.read()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_shape(&mut self, shape: LfoShape) -> bool {
        self.tx.push(UiEvent::Shape(shape)).is_ok()
    }

    pub fn set_bipolar(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::Bipolar(value)).is_ok()
    }

    pub fn set_steal_phase(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::StealPhase(value)).is_ok()
    }

    pub fn set_smooth_time(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::SmoothTime(value)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    phase: triple_buffer::Input<Sample>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_phase(&mut self, phase: Sample) {
        self.phase.write(phase);
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(128);
    let (phase_input, phase_output) = triple_buffer(&0.0);

    (
        AudioEnd {
            rx: from_ui_rx,
            phase: phase_input,
        },
        UiEnd {
            tx: to_audio_tx,
            phase: phase_output,
        },
    )
}
