
use crate::synth_engine::{Input, Sample, StereoSample};
use additizer_dsp::filters::spectral_filter::FilterType;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    FilterType(FilterType),
    LinearPhase(bool),
    Keytrack(Sample),
    QCutoff(StereoSample),
    QRolloff(StereoSample),
}

pub trait SpectralFilterAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_pitch(&mut self, pitch: Sample);
}

pub trait SpectralFilterUiEnd: Send {
    fn pitch(&mut self) -> Sample;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_filter_type(&mut self, filter_type: FilterType) -> bool;
    fn set_linear_phase(&mut self, value: bool) -> bool;
    fn set_keytrack(&mut self, value: Sample) -> bool;
    fn set_q_cutoff(&mut self, value: StereoSample) -> bool;
    fn set_q_rolloff(&mut self, value: StereoSample) -> bool;
}

pub trait SpectralFilterLinks: Send {
    type AudioEnd: SpectralFilterAudioEnd;
    type UiEnd: SpectralFilterUiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
