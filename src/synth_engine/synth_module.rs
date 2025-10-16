use crate::synth_engine::{
    buffer::{Buffer, HARMONIC_SERIES_BUFFER, SpectralBuffer, ZEROES_SPECTRAL_BUFFER},
    routing::{ModuleId, Router},
    types::Sample,
};

pub struct NoteOnParams {
    pub note: f32,
    pub velocity: f32,
    pub voice_idx: usize,
    pub same_note_retrigger: bool,
}

pub struct NoteOffParams {
    pub note: u8,
    pub voice_idx: usize,
}

pub struct ProcessParams<'a> {
    pub samples: usize,
    pub sample_rate: Sample,
    pub t_step: Sample,
    pub buffer_t_step: Sample,
    pub active_voices: &'a [usize],
}

pub trait SynthModule {
    fn get_id(&self) -> ModuleId;
    fn note_on(&mut self, params: &NoteOnParams);
    fn note_off(&mut self, params: &NoteOffParams);
    fn process(&mut self, params: &ProcessParams, router: &dyn Router);
}

pub trait BufferOutputModule: SynthModule {
    fn get_output(&self, voice_idx: usize, channel: usize) -> &Buffer;
}

pub struct SpectralOutputs<'a> {
    pub first: &'a SpectralBuffer,
    pub current: &'a SpectralBuffer,
}

impl SpectralOutputs<'_> {
    pub fn zero() -> Self {
        Self {
            first: &ZEROES_SPECTRAL_BUFFER,
            current: &ZEROES_SPECTRAL_BUFFER,
        }
    }

    pub fn harmonic() -> Self {
        Self {
            first: &HARMONIC_SERIES_BUFFER,
            current: &HARMONIC_SERIES_BUFFER,
        }
    }
}

pub trait SpectralOutputModule: SynthModule {
    fn get_output(&self, voice_idx: usize, channel: usize) -> SpectralOutputs<'_>;
}

pub struct ScalarOutputs {
    pub first: Sample,
    pub current: Sample,
}

impl ScalarOutputs {
    pub fn zero() -> Self {
        Self {
            first: 0.0,
            current: 0.0,
        }
    }

    pub fn one() -> Self {
        Self {
            first: 1.0,
            current: 1.0,
        }
    }
}

pub trait ScalarOutputModule: SynthModule {
    fn get_output(&self, voice_idx: usize, channel: usize) -> ScalarOutputs;
}
