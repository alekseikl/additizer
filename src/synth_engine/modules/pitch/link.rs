
use crate::synth_engine::{Input, Sample, StereoSample};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    Keytrack(bool),
    GlideAlways(bool),
    GlidePerOctave(bool),
}

pub trait PitchAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_pitch(&mut self, pitch: Sample);
}

pub trait PitchUiEnd: Send {
    fn get_pitch(&mut self) -> Sample;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_keytrack(&mut self, value: bool) -> bool;
    fn set_glide_always(&mut self, value: bool) -> bool;
    fn set_glide_per_octave(&mut self, value: bool) -> bool;
}

pub trait PitchLinks: Send {
    type AudioEnd: PitchAudioEnd;
    type UiEnd: PitchUiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
