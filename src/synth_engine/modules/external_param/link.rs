
use crate::synth_engine::Sample;

pub enum UiEvent {
    SelectedParamIndex(usize),
    Smooth(Sample),
    SampleOnTrigger(bool),
    MakeBipolar(bool),
    Polyphonic(bool),
}

pub trait ExternalParamAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_value(&mut self, value: Sample);
}

pub trait ExternalParamUiEnd: Send {
    fn get_value(&mut self) -> Sample;
    fn select_param(&mut self, index: usize) -> bool;
    fn set_smooth(&mut self, value: Sample) -> bool;
    fn set_sample_on_trigger(&mut self, value: bool) -> bool;
    fn set_make_bipolar(&mut self, value: bool) -> bool;
    fn set_polyphonic(&mut self, value: bool) -> bool;
}

pub trait ExternalParamLinks: Send {
    type AudioEnd: ExternalParamAudioEnd;
    type UiEnd: ExternalParamUiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
