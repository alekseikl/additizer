use triple_buffer::triple_buffer;

use addi_dsp::segments::time_segments::TimeSegments;
use addi_engine::multi_envelope::{
    MultiEnvelopeAudioEnd, MultiEnvelopeLinks, MultiEnvelopePhase, MultiEnvelopeUiEnd, UiEvent,
    UiUpdate,
};
use addi_engine::{AUDIO_TO_UI_RING_CAPACITY, UI_TO_AUDIO_RING_CAPACITY};
use addi_engine::{Input, NUM_CHANNELS, Smp, StereoSample};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    rx: rtrb::Consumer<UiUpdate>,
    phase: triple_buffer::Output<MultiEnvelopePhase>,
    segments: triple_buffer::Input<TimeSegments>,
}

impl UiEnd {
    pub fn get_phase(&mut self) -> MultiEnvelopePhase {
        *self.phase.read()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_segments(&mut self, segments: TimeSegments) -> bool {
        self.segments.write(segments);
        self.tx.push(UiEvent::SegmentsUpdated).is_ok()
    }

    pub fn set_steal_level(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::StealLevel(value)).is_ok()
    }

    pub fn set_keep_voice_alive(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::KeepVoiceAlive(value)).is_ok()
    }

    pub fn pop_update(&mut self) -> Option<UiUpdate> {
        self.rx.pop().ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    tx: rtrb::Producer<UiUpdate>,
    phase: triple_buffer::Input<MultiEnvelopePhase>,
    segments: triple_buffer::Output<TimeSegments>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_phase(&mut self, channel: usize, time: Smp, level: Smp) {
        let phase = self.phase.input_buffer_mut();

        phase.time[channel] = time;
        phase.level[channel] = level;

        if channel == NUM_CHANNELS - 1 {
            self.phase.publish();
        }
    }

    pub fn read_segments(&mut self) -> &TimeSegments {
        self.segments.read()
    }

    pub fn refresh_routing(&mut self) -> bool {
        self.tx.push(UiUpdate::RefreshRouting).is_ok()
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(AUDIO_TO_UI_RING_CAPACITY);
    let (phase_input, phase_output) = triple_buffer(&MultiEnvelopePhase::default());
    let (segments_input, segments_output) = triple_buffer(&TimeSegments::default());

    (
        AudioEnd {
            rx: from_ui_rx,
            tx: to_ui_tx,
            phase: phase_input,
            segments: segments_output,
        },
        UiEnd {
            tx: to_audio_tx,
            rx: from_audio_rx,
            phase: phase_output,
            segments: segments_input,
        },
    )
}

impl MultiEnvelopeAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_phase(&mut self, channel: usize, time: Smp, level: Smp) {
        AudioEnd::update_phase(self, channel, time, level)
    }
    fn read_segments(&mut self) -> &TimeSegments {
        AudioEnd::read_segments(self)
    }
    fn refresh_routing(&mut self) -> bool {
        AudioEnd::refresh_routing(self)
    }
}

impl MultiEnvelopeUiEnd for UiEnd {
    fn get_phase(&mut self) -> MultiEnvelopePhase {
        UiEnd::get_phase(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_segments(&mut self, segments: TimeSegments) -> bool {
        UiEnd::set_segments(self, segments)
    }
    fn set_steal_level(&mut self, value: bool) -> bool {
        UiEnd::set_steal_level(self, value)
    }
    fn set_keep_voice_alive(&mut self, value: bool) -> bool {
        UiEnd::set_keep_voice_alive(self, value)
    }
    fn pop_update(&mut self) -> Option<UiUpdate> {
        UiEnd::pop_update(self)
    }
}

pub struct Links;

impl MultiEnvelopeLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
