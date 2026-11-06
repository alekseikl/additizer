use std::sync::LazyLock;

use addi_dsp::segments::time_segments::TimeSegments;

use crate::synth_engine::Smp;

use super::{
    MultiEnvelopeAudioEnd, MultiEnvelopeLinks, MultiEnvelopePhase, MultiEnvelopeUiEnd, UiEvent,
    UiUpdate,
};

static SEGMENTS: LazyLock<TimeSegments> = LazyLock::new(TimeSegments::default);

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl MultiEnvelopeLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl MultiEnvelopeAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_phase(&mut self, _channel: usize, _time: Smp, _level: Smp) {}

    fn read_segments(&mut self) -> &TimeSegments {
        &SEGMENTS
    }

    fn refresh_routing(&mut self) -> bool {
        false
    }
}

impl MultiEnvelopeUiEnd for NoUi {
    fn get_phase(&mut self) -> MultiEnvelopePhase {
        self.diverge()
    }

    fn set_segments(&mut self, _segments: TimeSegments) -> bool {
        self.diverge()
    }

    fn set_steal_level(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_keep_voice_alive(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn pop_update(&mut self) -> Option<UiUpdate> {
        self.diverge()
    }
}
