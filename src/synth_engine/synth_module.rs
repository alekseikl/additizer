use std::{any::Any, sync::Arc};

use parking_lot::Mutex;

use crate::synth_engine::{
    buffer::{Buffer, HARMONIC_SERIES_BUFFER, SpectralBuffer, ZEROES_SPECTRAL_BUFFER},
    routing::{InputType, ModuleId, ModuleType, OutputType, Router},
    types::Sample,
};

pub struct NoteOnParams {
    pub note: f32,
    // pub velocity: f32,
    pub voice_idx: usize,
    pub same_note_retrigger: bool,
}

pub struct NoteOffParams {
    // pub note: u8,
    pub voice_idx: usize,
}

pub struct ProcessParams<'a> {
    pub samples: usize,
    pub sample_rate: Sample,
    pub t_step: Sample,
    // pub buffer_t_step: Sample,
    pub active_voices: &'a [usize],
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

    // pub fn one() -> Self {
    //     Self {
    //         first: 1.0,
    //         current: 1.0,
    //     }
    // }
}

#[allow(unused_variables)]
pub trait SynthModule: Any + Send {
    fn id(&self) -> ModuleId;
    fn module_type(&self) -> ModuleType;
    fn inputs(&self) -> &'static [InputType];
    fn outputs(&self) -> &'static [OutputType];
    fn note_on(&mut self, params: &NoteOnParams) {}
    fn note_off(&mut self, params: &NoteOffParams) {}
    fn process(&mut self, params: &ProcessParams, router: &dyn Router);

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        panic!("{:?} don't have buffer output.", self.module_type())
    }

    fn get_spectral_output(&self, voice_idx: usize, channel: usize) -> SpectralOutputs<'_> {
        panic!("{:?} don't have spectral output.", self.module_type())
    }

    fn get_scalar_output(&self, voice_idx: usize, channel: usize) -> ScalarOutputs {
        panic!("{:?} don't have scalar output.", self.module_type())
    }
}

pub type ModuleConfigBox<T> = Arc<Mutex<T>>;
