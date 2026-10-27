use std::sync::LazyLock;

use super::{ModMapAudioEnd, ModMapLinks, ModMapUiEnd, UiEvent};

use addi_dsp::segments::Segments;

use crate::synth_engine::{Input, Smp, StereoSample};

static SEGMENTS: LazyLock<Segments> =
    LazyLock::new(|| Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl ModMapLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl ModMapAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn read_segments(&mut self) -> &Segments {
        &SEGMENTS
    }
}

impl ModMapUiEnd for NoUi {
    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_segments(&mut self, _segments: Segments) -> bool {
        self.diverge()
    }

    fn set_bipolar(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_smooth(&mut self, _value: Smp) -> bool {
        self.diverge()
    }
}
