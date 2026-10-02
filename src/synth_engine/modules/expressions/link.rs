use crate::synth_engine::{Expression, Sample, engine_io};

pub enum UiEvent {
    Expression(Expression),
    UseReleaseVelocity(bool),
    Smooth(Sample),
}

pub trait ExpressionsAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_value(&mut self, value: Sample);
}

pub trait ExpressionsUiEnd: Send {
    fn get_value(&mut self) -> Sample;
    fn set_expression(&mut self, expression: Expression) -> bool;
    fn set_use_release_velocity(&mut self, value: bool) -> bool;
    fn set_smooth(&mut self, value: Sample) -> bool;
}

pub trait ExpressionsLinks: Send {
    type AudioEnd: ExpressionsAudioEnd;
    type UiEnd: ExpressionsUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
