use crate::synth_engine::{Input, Sample, StereoSample, engine_io};

use super::config::EqFilter;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    LinearPhase(bool),
    Keytrack(Sample),
    QCutoff(StereoSample),
    QRolloff(StereoSample),
    SetFilter { index: u8, filter: EqFilter },
    AddFilter(EqFilter),
    RemoveFilter(u8),
    MoveFilter { from: u8, to: u8 },
}

pub trait SpectralEqAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_pitch(&mut self, pitch: Sample);
}

pub trait SpectralEqUiEnd: Send {
    fn pitch(&mut self) -> Sample;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_linear_phase(&mut self, value: bool) -> bool;
    fn set_keytrack(&mut self, value: Sample) -> bool;
    fn set_q_cutoff(&mut self, value: StereoSample) -> bool;
    fn set_q_rolloff(&mut self, value: StereoSample) -> bool;
    fn set_filter(&mut self, index: u8, filter: EqFilter) -> bool;
    fn add_filter(&mut self, filter: EqFilter) -> bool;
    fn remove_filter(&mut self, index: u8) -> bool;
    fn move_filter(&mut self, from: u8, to: u8) -> bool;
}

pub trait SpectralEqLinks: Send {
    type AudioEnd: SpectralEqAudioEnd;
    type UiEnd: SpectralEqUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
