use crate::{
    buffer::Buffer,
    synth_engine::routing::{ModuleId, Router},
};

pub struct NoteOnParams {
    pub note: f32,
    pub velocity: f32,
    pub voice_idx: usize,
    pub same_note_retrigger: bool,
    pub initial_phase: u32,
}

pub struct NoteOffParams {
    pub note: u8,
    pub voice_idx: usize,
}

pub struct ProcessParams {
    pub samples: usize,
    pub sample_rate: f32,
    pub active_voices: Vec<usize>,
}

pub trait SynthModule {
    fn get_id(&self) -> ModuleId;
    fn get_output(&self, voice_idx: usize) -> &Buffer;
    fn note_on(&mut self, params: &NoteOnParams);
    fn note_off(&mut self, params: &NoteOffParams);
    fn process(&mut self, params: &ProcessParams, router: &dyn Router);
}
