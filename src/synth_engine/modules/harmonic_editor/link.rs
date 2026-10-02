use std::array;


use crate::synth_engine::{
    ComplexSample, DisplaySpectrum, NUM_CHANNELS, SPECTRAL_BUFFER_SIZE, Sample, StereoSample,
    harmonic_editor::EditRequest,
};

#[derive(Clone)]
pub struct Harmonics {
    amplitudes: [Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
    phases: [Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
}

impl Default for Harmonics {
    fn default() -> Self {
        Self {
            amplitudes: array::from_fn(|_| Box::new([0.0; SPECTRAL_BUFFER_SIZE])),
            phases: array::from_fn(|_| Box::new([0.0; SPECTRAL_BUFFER_SIZE])),
        }
    }
}

impl Harmonics {
    pub fn amplitude(&self, index: usize) -> StereoSample {
        StereoSample::new(self.amplitudes[0][index], self.amplitudes[1][index])
    }

    pub fn set_amplitude(&mut self, index: usize, gain: StereoSample) {
        for (amplitudes, &gain) in self.amplitudes.iter_mut().zip(gain.iter()) {
            amplitudes[index] = gain;
        }
    }

    pub fn phase(&self, index: usize) -> StereoSample {
        StereoSample::new(self.phases[0][index], self.phases[1][index])
    }

    pub fn set_phase(&mut self, index: usize, phase: StereoSample) {
        for (phases, &phase) in self.phases.iter_mut().zip(phase.iter()) {
            phases[index] = phase;
        }
    }

    pub(crate) fn copy_channels(
        &mut self,
        amplitudes: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
        phases: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
    ) {
        for (dst, src) in self.amplitudes.iter_mut().zip(amplitudes.iter()) {
            dst.copy_from_slice(src.as_slice());
        }

        for (dst, src) in self.phases.iter_mut().zip(phases.iter()) {
            dst.copy_from_slice(src.as_slice());
        }
    }
}

pub enum UiEvent {
    SetAmplitude { index: u32, gain: StereoSample },
    SetPhase { index: u32, phase: StereoSample },
    Clear,
    ResetSawtooth,
    ZeroPhases,
    SawtoothPhases,
    RandomPhases,
    RandomPhasesStereo,
    EditRequest(EditRequest),
    ApplyDraft,
    DiscardDraft,
    Bandwidth(i32),
    Mono(bool),
    CaptureInput(bool),
}

pub trait HarmonicEditorAudioEnd: Send {
    fn pop_event(&mut self) -> Option<UiEvent>;
    fn update_display_spectrum(&mut self, spectrum: &[ComplexSample]);
    fn publish_harmonics(&mut self, amplitudes: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS], phases: &[Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS]);
}

pub trait HarmonicEditorUiEnd: Send {
    fn get_display_spectrum(&mut self) -> &DisplaySpectrum;
    fn get_harmonics_mut(&mut self) -> &mut Harmonics;
    fn set_amplitude(&mut self, index: usize, gain: StereoSample) -> bool;
    fn set_phase(&mut self, index: usize, phase: StereoSample) -> bool;
    fn clear(&mut self) -> bool;
    fn reset_sawtooth(&mut self) -> bool;
    fn zero_phases(&mut self) -> bool;
    fn sawtooth_phases(&mut self) -> bool;
    fn random_phases(&mut self) -> bool;
    fn random_phases_stereo(&mut self) -> bool;
    fn edit_request(&mut self, request: EditRequest) -> bool;
    fn apply_draft(&mut self) -> bool;
    fn discard_draft(&mut self) -> bool;
    fn set_bandwidth(&mut self, bandwidth: i32) -> bool;
    fn set_mono(&mut self, mono: bool) -> bool;
    fn set_capture_input(&mut self, capture_input: bool) -> bool;
}

pub trait HarmonicEditorLinks: Send {
    type AudioEnd: HarmonicEditorAudioEnd;
    type UiEnd: HarmonicEditorUiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
