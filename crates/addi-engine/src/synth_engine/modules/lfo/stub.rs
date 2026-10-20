use std::sync::LazyLock;

use super::{LfoAudioEnd, LfoLinks, LfoUiEnd, UiEvent};

use addi_dsp::segments::Segments;

use crate::synth_engine::{Input, Smp, StereoSample};

static SEGMENTS: LazyLock<Segments> = LazyLock::new(Segments::default);

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

    fn update_phase(&mut self, _phase: Smp) {}

    fn read_segments(&mut self) -> &Segments {
        &SEGMENTS
    }
}

impl LfoUiEnd for NoUi {
    fn get_phase(&mut self) -> Smp {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_segments(&mut self, _segments: Segments) -> bool {
        self.diverge()
    }

    fn set_steal_phase(&mut self, _value: bool) -> bool {
        self.diverge()
    }
}
