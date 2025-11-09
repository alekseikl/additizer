use std::{any::Any, sync::Arc};

use parking_lot::Mutex;

use crate::synth_engine::{
    buffer::{Buffer, SpectralBuffer},
    routing::{InputType, ModuleId, ModuleType, OutputType, Router},
    types::Sample,
};

pub struct NoteOnParams {
    pub sample_rate: Sample,
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
    pub buffer_t_step: Sample,
    pub active_voices: &'a [usize],
}

#[allow(unused_variables)]
pub trait SynthModule: Any + Send {
    fn id(&self) -> ModuleId;
    fn module_type(&self) -> ModuleType;

    fn is_spectral_rate(&self) -> bool;

    fn label(&self) -> String {
        format!("{:?} {}", self.module_type(), self.id())
    }

    fn inputs(&self) -> &'static [InputType];
    fn output_type(&self) -> OutputType;

    fn note_on(&mut self, params: &NoteOnParams, router: &dyn Router) {}
    fn note_off(&mut self, params: &NoteOffParams) {}
    fn process(&mut self, params: &ProcessParams, router: &dyn Router);

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        panic!("{:?} don't have buffer output.", self.module_type())
    }

    fn get_spectral_output(&self, voice_idx: usize, channel_idx: usize) -> &SpectralBuffer {
        panic!("{:?} don't have spectral output.", self.module_type())
    }

    fn get_scalar_output(&self, voice_idx: usize, channel_idx: usize) -> (Sample, Sample) {
        panic!("{:?} don't have scalar output.", self.module_type())
    }
}

pub type ModuleConfigBox<T> = Arc<Mutex<T>>;

macro_rules! gen_downcast_methods {
    ($mod_type:ident) => {
        pub fn downcast(module: &dyn SynthModule) -> Option<&$mod_type> {
            (module as &dyn Any).downcast_ref()
        }

        pub fn downcast_mut(module: &mut dyn SynthModule) -> Option<&mut $mod_type> {
            (module as &mut dyn Any).downcast_mut()
        }

        pub fn downcast_mut_unwrap(module: Option<&mut dyn SynthModule>) -> &mut $mod_type {
            Self::downcast_mut(module.unwrap()).unwrap()
        }
    };
}
