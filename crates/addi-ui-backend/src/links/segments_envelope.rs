use triple_buffer::triple_buffer;

use addi_dsp::segments::Segments;
use addi_engine::segments_envelope::{
    SegmentsEnvelopeAudioEnd, SegmentsEnvelopeLinks, SegmentsEnvelopePhase, SegmentsEnvelopeUiEnd,
    Timebase, UiEvent,
};
use addi_engine::{Input, StereoSample, UI_TO_AUDIO_RING_CAPACITY};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    phase: triple_buffer::Output<SegmentsEnvelopePhase>,
    segments: triple_buffer::Input<Segments>,
}

impl UiEnd {
    pub fn get_phase(&mut self) -> SegmentsEnvelopePhase {
        *self.phase.read()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_segments(&mut self, segments: Segments) -> bool {
        self.segments.write(segments);
        self.tx.push(UiEvent::SegmentsUpdated).is_ok()
    }

    pub fn set_steal_level(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::StealLevel(value)).is_ok()
    }

    pub fn set_keep_voice_alive(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::KeepVoiceAlive(value)).is_ok()
    }

    pub fn set_timebase(&mut self, timebase: Timebase) -> bool {
        self.tx.push(UiEvent::Timebase(timebase)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    phase: triple_buffer::Input<SegmentsEnvelopePhase>,
    segments: triple_buffer::Output<Segments>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_phase(&mut self, phase: SegmentsEnvelopePhase) {
        self.phase.write(phase);
    }

    pub fn read_segments(&mut self) -> &Segments {
        self.segments.read()
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (phase_input, phase_output) = triple_buffer(&SegmentsEnvelopePhase::default());
    let (segments_input, segments_output) = triple_buffer(&Segments::default());

    (
        AudioEnd {
            rx: from_ui_rx,
            phase: phase_input,
            segments: segments_output,
        },
        UiEnd {
            tx: to_audio_tx,
            phase: phase_output,
            segments: segments_input,
        },
    )
}

impl SegmentsEnvelopeAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_phase(&mut self, phase: SegmentsEnvelopePhase) {
        AudioEnd::update_phase(self, phase)
    }
    fn read_segments(&mut self) -> &Segments {
        AudioEnd::read_segments(self)
    }
}

impl SegmentsEnvelopeUiEnd for UiEnd {
    fn get_phase(&mut self) -> SegmentsEnvelopePhase {
        UiEnd::get_phase(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_segments(&mut self, segments: Segments) -> bool {
        UiEnd::set_segments(self, segments)
    }
    fn set_steal_level(&mut self, value: bool) -> bool {
        UiEnd::set_steal_level(self, value)
    }
    fn set_keep_voice_alive(&mut self, value: bool) -> bool {
        UiEnd::set_keep_voice_alive(self, value)
    }
    fn set_timebase(&mut self, timebase: Timebase) -> bool {
        UiEnd::set_timebase(self, timebase)
    }
}

pub struct Links;

impl SegmentsEnvelopeLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
