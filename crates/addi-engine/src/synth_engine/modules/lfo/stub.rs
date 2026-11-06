use std::sync::LazyLock;

use super::{LfoAudioEnd, LfoLinks, LfoUiEnd, Timebase, UiEvent};

use addi_dsp::segments::rel_segments::RelSegments;

use crate::synth_engine::{Input, StereoSample};

static SEGMENTS: LazyLock<RelSegments> = LazyLock::new(RelSegments::default);

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl LfoLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl LfoAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_phase(&mut self, _phase: StereoSample) {}

    fn read_segments(&mut self) -> &RelSegments {
        &SEGMENTS
    }
}

impl LfoUiEnd for NoUi {
    fn get_phase(&mut self) -> StereoSample {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_segments(&mut self, _segments: RelSegments) -> bool {
        self.diverge()
    }

    fn set_steal_phase(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_timebase(&mut self, _timebase: Timebase) -> bool {
        self.diverge()
    }
}
