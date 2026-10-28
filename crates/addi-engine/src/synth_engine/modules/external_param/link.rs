use crate::synth_engine::{Smp, engine_io};

pub enum UiEvent {
    SelectedParamIndex(usize),
    SampleOnTrigger(bool),
    Polyphonic(bool),
}

pub trait ExternalParamAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_value(&mut self, value: Smp);
}

pub trait ExternalParamUiEnd: Send {
    fn get_value(&mut self) -> Smp;
    fn select_param(&mut self, index: usize) -> bool;
    fn set_sample_on_trigger(&mut self, value: bool) -> bool;
    fn set_polyphonic(&mut self, value: bool) -> bool;
}

pub trait ExternalParamLinks: Send {
    type AudioEnd: ExternalParamAudioEnd;
    type UiEnd: ExternalParamUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
