use std::sync::LazyLock;

use super::{
    SegmentsEnvelopeAudioEnd, SegmentsEnvelopeLinks, SegmentsEnvelopePhase, SegmentsEnvelopeUiEnd,
    Timebase, UiEvent,
};

use addi_dsp::segments::rel_segments::RelSegments;

use crate::synth_engine::{Input, Smp, StereoSample};

static SEGMENTS: LazyLock<RelSegments> = LazyLock::new(RelSegments::default);

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl SegmentsEnvelopeLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl SegmentsEnvelopeAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_phase(&mut self, _channel: usize, _phase: Smp, _level: Smp) {}

    fn read_segments(&mut self) -> &RelSegments {
        &SEGMENTS
    }
}

impl SegmentsEnvelopeUiEnd for NoUi {
    fn get_phase(&mut self) -> SegmentsEnvelopePhase {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_segments(&mut self, _segments: RelSegments) -> bool {
        self.diverge()
    }

    fn set_steal_level(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_keep_voice_alive(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_timebase(&mut self, _timebase: Timebase) -> bool {
        self.diverge()
    }
}
