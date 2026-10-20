use crate::synth_engine::{Input, Smp, StereoSample, engine_io};

use super::config::LfoShape;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    Shape(LfoShape),
    StealPhase(bool),
    Skew(Smp),
}

pub trait LfoAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_phase(&mut self, phase: Smp);
}

pub trait LfoUiEnd: Send {
    fn get_phase(&mut self) -> Smp;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_shape(&mut self, shape: LfoShape) -> bool;
    fn set_steal_phase(&mut self, value: bool) -> bool;
    fn set_skew(&mut self, value: Smp) -> bool;
}

pub trait LfoLinks: Send {
    type AudioEnd: LfoAudioEnd;
    type UiEnd: LfoUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
