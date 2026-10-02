
use crate::synth_engine::{Input, Sample, StereoSample};

use super::config::LfoShape;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    Shape(LfoShape),
    Bipolar(bool),
    StealPhase(bool),
    SmoothTime(StereoSample),
}

pub trait LfoAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_phase(&mut self, phase: Sample);
}

pub trait LfoUiEnd: Send {
    fn get_phase(&mut self) -> Sample;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_shape(&mut self, shape: LfoShape) -> bool;
    fn set_bipolar(&mut self, value: bool) -> bool;
    fn set_steal_phase(&mut self, value: bool) -> bool;
    fn set_smooth_time(&mut self, value: StereoSample) -> bool;
}

pub trait LfoLinks: Send {
    type AudioEnd: LfoAudioEnd;
    type UiEnd: LfoUiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
