use triple_buffer::triple_buffer;

use crate::synth_engine::{Input, Sample, StereoSample};

use super::EnvelopePhase;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    AttackCurvature(Sample),
    DecayCurvature(Sample),
    ReleaseCurvature(Sample),
    KeepVoiceAlive(bool),
}

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    phase: triple_buffer::Output<EnvelopePhase>,
}

impl UiEnd {
    pub fn get_phase(&mut self) -> EnvelopePhase {
        *self.phase.read()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_attack_curvature(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::AttackCurvature(value)).is_ok()
    }

    pub fn set_decay_curvature(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::DecayCurvature(value)).is_ok()
    }

    pub fn set_release_curvature(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::ReleaseCurvature(value)).is_ok()
    }

    pub fn set_keep_voice_alive(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::KeepVoiceAlive(value)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    phase: triple_buffer::Input<EnvelopePhase>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_phase(&mut self, phase: EnvelopePhase) {
        self.phase.write(phase);
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(128);
    let (phase_input, phase_output) = triple_buffer(&EnvelopePhase::default());

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
