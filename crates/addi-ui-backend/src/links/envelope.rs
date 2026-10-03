use triple_buffer::triple_buffer;

use addi_engine::{Input, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY};

use addi_engine::envelope::EnvelopePhase;

use addi_engine::envelope::{EnvelopeAudioEnd, EnvelopeLinks, EnvelopeUiEnd, UiEvent};

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

    pub fn set_attack_slope(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::AttackSlope(value)).is_ok()
    }

    pub fn set_decay_slope(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::DecaySlope(value)).is_ok()
    }

    pub fn set_release_slope(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::ReleaseSlope(value)).is_ok()
    }

    pub fn set_keep_voice_alive(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::KeepVoiceAlive(value)).is_ok()
    }

    pub fn set_steal_level(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::StealLevel(value)).is_ok()
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

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
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

impl EnvelopeAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_phase(&mut self, phase: EnvelopePhase) {
        AudioEnd::update_phase(self, phase)
    }
}

impl EnvelopeUiEnd for UiEnd {
    fn get_phase(&mut self) -> EnvelopePhase {
        UiEnd::get_phase(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_attack_slope(&mut self, value: Sample) -> bool {
        UiEnd::set_attack_slope(self, value)
    }
    fn set_decay_slope(&mut self, value: Sample) -> bool {
        UiEnd::set_decay_slope(self, value)
    }
    fn set_release_slope(&mut self, value: Sample) -> bool {
        UiEnd::set_release_slope(self, value)
    }
    fn set_keep_voice_alive(&mut self, value: bool) -> bool {
        UiEnd::set_keep_voice_alive(self, value)
    }
    fn set_steal_level(&mut self, value: bool) -> bool {
        UiEnd::set_steal_level(self, value)
    }
}

pub struct Links;

impl EnvelopeLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
