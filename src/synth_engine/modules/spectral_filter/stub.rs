use additizer_dsp::filters::spectral_filter::FilterType;

use super::{SpectralFilterAudioEnd, SpectralFilterLinks, SpectralFilterUiEnd, UiEvent};

use crate::synth_engine::{Input, Sample, StereoSample};

pub struct AudioEnd;

pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

pub struct Links;

impl SpectralFilterLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type EngineEnd = crate::synth_engine::stub::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl SpectralFilterAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_pitch(&mut self, _pitch: Sample) {}
}

impl SpectralFilterUiEnd for NoUi {
    fn pitch(&mut self) -> Sample {
        self.diverge()
    }

    fn set_param(&mut self, _input: Input, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_filter_type(&mut self, _filter_type: FilterType) -> bool {
        self.diverge()
    }

    fn set_linear_phase(&mut self, _value: bool) -> bool {
        self.diverge()
    }

    fn set_keytrack(&mut self, _value: Sample) -> bool {
        self.diverge()
    }

    fn set_q_cutoff(&mut self, _value: StereoSample) -> bool {
        self.diverge()
    }

    fn set_q_rolloff(&mut self, _value: StereoSample) -> bool {
        self.diverge()
    }
}
