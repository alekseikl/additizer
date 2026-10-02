use super::{EnvelopeAudioEnd, EnvelopeLinks, EnvelopePhase, EnvelopeUiEnd, UiEvent};

use crate::synth_engine::{Input, Sample, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl EnvelopeLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl EnvelopeAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_phase(&mut self, _phase: EnvelopePhase) {}
}

impl EnvelopeUiEnd for NoUi {
    fn get_phase(&mut self) -> EnvelopePhase {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_attack_slope(&mut self, _value: Sample) -> bool {
        self.diverge()
    }

    fn set_decay_slope(&mut self, _value: Sample) -> bool {
        self.diverge()
    }

    fn set_release_slope(&mut self, _value: Sample) -> bool {
        self.diverge()
    }

    fn set_keep_voice_alive(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_steal_level(&mut self, _value: bool) -> bool {
        self.diverge()
    }
}
