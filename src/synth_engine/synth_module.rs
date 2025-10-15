use crate::synth_engine::{
    buffer::{Buffer, SpectralBuffer},
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
    pub sample_rate: f32,
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

pub trait SpectralOutputModule: SynthModule {
    fn get_output(&self, voice_idx: usize, channel: usize) -> &SpectralBuffer;
}

pub trait ScalarOutputModule: SynthModule {
    fn get_output(&self, voice_idx: usize, channel: usize) -> &Sample;
}
