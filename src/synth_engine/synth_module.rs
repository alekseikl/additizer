use std::{any::Any, sync::Arc};

use parking_lot::Mutex;

use crate::synth_engine::{
    ModuleInput,
    buffer::{Buffer, SpectralBuffer, ZEROES_BUFFER, ZEROES_SPECTRAL_BUFFER},
    routing::{InputType, ModuleId, ModuleType, OutputType, Router},
    types::Sample,
};

pub struct NoteOnParams {
    pub note: f32,
    pub voice_idx: usize,
    pub reset: bool,
}

pub struct NoteOffParams {
    pub voice_idx: usize,
}

pub struct ProcessParams<'a> {
    pub samples: usize,
    pub sample_rate: Sample,
    pub buffer_t_step: Sample,
    pub active_voices: &'a [usize],
}

#[allow(unused_variables)]
pub trait SynthModule: Any + Send {
    fn id(&self) -> ModuleId;
    fn module_type(&self) -> ModuleType;
    fn label(&self) -> String;
    fn set_label(&mut self, label: String);

    fn inputs(&self) -> &'static [InputType];
    fn output_type(&self) -> OutputType;

    fn note_on(&mut self, params: &NoteOnParams) {}
    fn note_off(&mut self, params: &NoteOffParams) {}
    fn process(&mut self, params: &ProcessParams, router: &dyn Router);

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        panic!("{:?} don't have buffer output.", self.module_type())
    }

    fn get_spectral_output(
        &self,
        current: bool,
        voice_idx: usize,
        channel_idx: usize,
    ) -> &SpectralBuffer {
        panic!("{:?} don't have spectral output.", self.module_type())
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel_idx: usize) -> Sample {
        panic!("{:?} don't have scalar output.", self.module_type())
    }
}

pub struct VoiceRouter<'a> {
    pub router: &'a dyn Router,
    pub samples: usize,
    pub voice_idx: usize,
    pub channel_idx: usize,
}

impl<'a> VoiceRouter<'a> {
    pub fn get_input(&'a self, input: ModuleInput, input_buffer: &'a mut Buffer) -> &'a Buffer {
        self.router
            .get_input(
                input,
                self.samples,
                self.voice_idx,
                self.channel_idx,
                input_buffer,
            )
            .unwrap_or(&ZEROES_BUFFER)
    }

    pub fn get_spectral_input(&self, input: ModuleInput, current: bool) -> &SpectralBuffer {
        self.router
            .get_spectral_input(input, current, self.voice_idx, self.channel_idx)
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER)
    }

    pub fn get_scalar_input(&self, input: ModuleInput, current: bool) -> Sample {
        self.router
            .get_scalar_input(input, current, self.voice_idx, self.channel_idx)
            .unwrap_or(0.0)
    }
}

pub type ModuleConfigBox<T> = Arc<Mutex<T>>;

macro_rules! gen_downcast_methods {
    ($mod_type:ident) => {
        #[allow(dead_code)]
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
