use crate::synth_engine::{Expression, Smp, engine_io};

pub enum UiEvent {
    Expression(Expression),
    UseReleaseVelocity(bool),
}

pub trait ExpressionsAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_value(&mut self, value: Smp);
}

pub trait ExpressionsUiEnd: Send {
    fn get_value(&mut self) -> Smp;
    fn set_expression(&mut self, expression: Expression) -> bool;
    fn set_use_release_velocity(&mut self, value: bool) -> bool;
}

pub trait ExpressionsLinks: Send {
    type AudioEnd: ExpressionsAudioEnd;
    type UiEnd: ExpressionsUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
