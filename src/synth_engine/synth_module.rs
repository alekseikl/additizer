use std::{any::Any, sync::Arc};

use parking_lot::Mutex;

use crate::synth_engine::{
    ModuleInput,
    buffer::{Buffer, SpectralBuffer, ZEROES_BUFFER, ZEROES_SPECTRAL_BUFFER},
    routing::{DataType, Input, ModuleId, ModuleType, Router},
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

pub struct InputInfo {
    pub input: Input,
    pub data_type: DataType,
}

impl InputInfo {
    pub const fn buffer(input: Input) -> Self {
        Self {
            input,
            data_type: DataType::Buffer,
        }
    }

    pub const fn scalar(input: Input) -> Self {
        Self {
            input,
            data_type: DataType::Scalar,
        }
    }

    pub const fn spectral(input: Input) -> Self {
        Self {
            input,
            data_type: DataType::Spectral,
        }
    }
}

#[allow(unused_variables)]
pub trait SynthModule: Any + Send {
    fn id(&self) -> ModuleId;
    fn module_type(&self) -> ModuleType;
    fn label(&self) -> String;
    fn set_label(&mut self, label: String);

    fn inputs(&self) -> &'static [InputInfo];
    fn output_type(&self) -> DataType;

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
    pub module_id: ModuleId,
    pub samples: usize,
    pub voice_idx: usize,
    pub channel_idx: usize,
}

impl<'a> VoiceRouter<'a> {
    pub fn buffer(&'a self, input: Input, buff: &'a mut Buffer) -> &'a Buffer {
        self.router
            .get_input(
                ModuleInput::new(input, self.module_id),
                self.samples,
                self.voice_idx,
                self.channel_idx,
                buff,
            )
            .unwrap_or(&ZEROES_BUFFER)
    }

    pub fn spectral(&self, input: Input, current: bool) -> &SpectralBuffer {
        self.router
            .get_spectral_input(
                ModuleInput::new(input, self.module_id),
                current,
                self.voice_idx,
                self.channel_idx,
            )
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER)
    }

    pub fn scalar(&self, input: Input, current: bool) -> Sample {
        self.router
            .get_scalar_input(
                ModuleInput::new(input, self.module_id),
                current,
                self.voice_idx,
                self.channel_idx,
            )
            .unwrap_or(0.0)
    }
}

pub type ModuleConfigBox<T> = Arc<Mutex<T>>;

macro_rules! gen_downcast_methods {
    () => {
        #[allow(dead_code)]
        pub fn downcast(module: &dyn SynthModule) -> Option<&Self> {
            (module as &dyn Any).downcast_ref()
        }

        pub fn downcast_mut(module: &mut dyn SynthModule) -> Option<&mut Self> {
            (module as &mut dyn Any).downcast_mut()
        }

        pub fn downcast_mut_unwrap(module: Option<&mut dyn SynthModule>) -> &mut Self {
            Self::downcast_mut(module.unwrap()).unwrap()
        }
    };
}

macro_rules! set_module_param_method {
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channels.iter_mut().zip($param.iter()) {
                channel.params.$param = $transform;
            }

            let mut cfg = self.config.lock();

            for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                config_channel.$param = channel.params.$param;
            }
        }
    };
}

macro_rules! extract_module_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channels.iter().map(|channel| channel.params.$param))
    };
}
