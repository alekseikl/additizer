use crate::synth_engine::{
    DisplaySpectrum, Input, Sample, StereoSample, engine_io, oscillator::PhasesDst,
    types::ComplexSample,
};

use super::MAX_UNISON_VOICES;

#[derive(Clone, Copy)]
pub struct Unison {
    pub initial_phases: [StereoSample; MAX_UNISON_VOICES],
    pub phase_shifts: [StereoSample; MAX_UNISON_VOICES],
    pub phase_shifts_to: [StereoSample; MAX_UNISON_VOICES],
    pub gains: [StereoSample; MAX_UNISON_VOICES],
    pub gains_to: [StereoSample; MAX_UNISON_VOICES],
}

impl Default for Unison {
    fn default() -> Self {
        Self {
            initial_phases: [StereoSample::ZERO; MAX_UNISON_VOICES],
            phase_shifts: [StereoSample::ZERO; MAX_UNISON_VOICES],
            phase_shifts_to: [StereoSample::ZERO; MAX_UNISON_VOICES],
            gains: [StereoSample::ONE; MAX_UNISON_VOICES],
            gains_to: [StereoSample::ONE; MAX_UNISON_VOICES],
        }
    }
}

pub enum UiEvent {
    InputParam {
        input: Input,
        value: StereoSample,
    },
    Unison(usize),
    UnisonInitialPhase {
        idx: usize,
        value: StereoSample,
    },
    UnisonPhaseShift {
        idx: usize,
        value: StereoSample,
    },
    UnisonPhaseShiftTo {
        idx: usize,
        value: StereoSample,
    },
    UnisonGain {
        idx: usize,
        value: StereoSample,
    },
    UnisonGainTo {
        idx: usize,
        value: StereoSample,
    },
    StealPhase(bool),
    PhaseRandom(Sample),
    PhaseRandomStereo(bool),
    MonoSpectrum(bool),
    ApplyUnisonLevelShape {
        center: StereoSample,
        level: StereoSample,
        to: bool,
    },
    RandomizePhases {
        amount: Sample,
        stereo_spread: Sample,
        dst: PhasesDst,
    },
}

pub trait OscillatorAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn publish_unison(&mut self, unison: &Unison);
    fn update_spectrum(&mut self, spectrum: &[ComplexSample]);
}

pub trait OscillatorUiEnd: Send {
    fn get_spectrum(&mut self) -> &DisplaySpectrum;
    fn get_unison_mut(&mut self) -> &mut Unison;
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool;
    fn set_unison(&mut self, unison: usize) -> bool;
    fn set_steal_phase(&mut self, steal_phase: bool) -> bool;
    fn set_phase_random(&mut self, phase_random: Sample) -> bool;
    fn set_phase_random_stereo(&mut self, phase_random_stereo: bool) -> bool;
    fn set_mono_spectrum(&mut self, mono_spectrum: bool) -> bool;
    fn set_unison_initial_phase(&mut self, idx: usize, value: StereoSample) -> bool;
    fn set_unison_phase_shift(&mut self, idx: usize, value: StereoSample) -> bool;
    fn set_unison_phase_shift_to(&mut self, idx: usize, value: StereoSample) -> bool;
    fn set_unison_gain(&mut self, idx: usize, value: StereoSample) -> bool;
    fn set_unison_gain_to(&mut self, idx: usize, value: StereoSample) -> bool;
    fn apply_unison_level_shape(
        &mut self,
        center: StereoSample,
        level: StereoSample,
        to: bool,
    ) -> bool;
    fn randomize_phases(&mut self, amount: Sample, stereo_spread: Sample, dst: PhasesDst) -> bool;
}

pub trait OscillatorLinks: Send {
    type AudioEnd: OscillatorAudioEnd;
    type UiEnd: OscillatorUiEnd;
    type EngineEnd: engine_io::EngineAudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
