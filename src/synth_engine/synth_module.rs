use std::any::Any;

use crate::synth_engine::{
    ModuleInput,
    buffer::{Buffer, SpectralBuffer, ZEROES_BUFFER},
    outputs_arena::{InputSlots, ProcessContext, SpectralInputSlot},
    routing::{DataType, Input, ModuleId, ModuleType, Router, VoiceEvent},
    smooth::SmoothedSampleParams,
    types::Sample,
    voices_handler::DecayingVoice,
};

pub struct ProcessParams<'a> {
    pub samples: usize,
    pub sample_rate: Sample,
    pub buffer_t_step: Sample,
    pub needs_update_ui: bool,
    pub smooth_params: SmoothedSampleParams,
    pub spectrum_channels: usize,
    pub active_voices: &'a [usize],
}

pub struct ModInput {
    pub input: Input,
    pub data_type: DataType,
}

impl ModInput {
    pub const fn audio(input: Input) -> Self {
        Self {
            input,
            data_type: DataType::Audio,
        }
    }

    pub const fn control(input: Input) -> Self {
        Self {
            input,
            data_type: DataType::Control,
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

    fn inputs(&self) -> &'static [ModInput];
    fn output(&self) -> DataType;

    fn set_slots(
        &mut self,
        inputs: &[InputSlots],
        spectral_inputs: &[SpectralInputSlot],
        output_slot: usize,
    ) {
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {}
    fn handle_ui_events(&mut self);
    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {}

    fn process(&mut self, params: &ProcessParams, router: &mut dyn Router) {}

    fn process2(&mut self, ctx: &mut ProcessContext) {}

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

pub trait ModuleUiBridge: Any + Send {
    fn update(&mut self);
}

pub struct VoiceRouterFactory<'a> {
    router: &'a mut dyn Router,
    process_params: &'a ProcessParams<'a>,
    module_id: ModuleId,
}

impl<'a> VoiceRouterFactory<'a> {
    pub fn new(
        module_id: ModuleId,
        router: &'a mut dyn Router,
        process_params: &'a ProcessParams,
    ) -> Self {
        Self {
            router,
            process_params,
            module_id,
        }
    }

    pub fn for_voice<'b>(
        &'b mut self,
        voice_idx: usize,
        channel_idx: usize,
        seq_idx: usize,
    ) -> VoiceRouter<'b, 'a>
    where
        'a: 'b,
    {
        VoiceRouter {
            factory: self,
            voice_idx,
            channel_idx,
            voice_seq_idx: seq_idx,
        }
    }
}

pub struct VoiceRouter<'a, 'b> {
    factory: &'a mut VoiceRouterFactory<'b>,
    voice_idx: usize,
    channel_idx: usize,
    voice_seq_idx: usize,
}

impl<'a, 'b> VoiceRouter<'a, 'b> {
    pub fn samples(&self) -> usize {
        self.factory.process_params.samples
    }

    pub fn channel_idx(&self) -> usize {
        self.channel_idx
    }

    pub fn voice_idx(&self) -> usize {
        self.voice_idx
    }

    pub fn buffer_opt(&'a self, input: Input, buff: &'a mut Buffer) -> Option<&'a Buffer> {
        self.factory.router.get_input(
            ModuleInput::new(input, self.factory.module_id),
            self.factory.process_params.samples,
            self.voice_idx,
            self.channel_idx,
            buff,
        )
    }

    pub fn buffer(&'a self, input: Input, buff: &'a mut Buffer) -> &'a Buffer {
        self.buffer_opt(input, buff).unwrap_or(&ZEROES_BUFFER)
    }
}

macro_rules! set_mono_param {
    ($fn_name:ident, $param:ident, $type:ty) => {
        pub fn $fn_name(&mut self, $param: $type) {
            self.params.$param = $param;
        }
    };
    ($fn_name:ident, $param:ident, $type:ty, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: $type) {
            self.params.$param = $transform;
        }
    };
}

macro_rules! set_stereo_param {
    ($fn_name:ident, $param:ident) => {
        set_stereo_param!($fn_name, $param, *$param);
    };
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channel_params.iter_mut().zip($param.iter()) {
                channel.$param = $transform;
            }
        }
    };
}

macro_rules! get_stereo_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channel_params.iter().map(|channel| channel.$param))
    };
}

macro_rules! set_smoothed_param {
    ($fn_name:ident, $param:ident) => {
        set_smoothed_param!($fn_name, $param, *$param);
    };
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channel_params.iter_mut().zip($param.iter()) {
                channel.$param.set($transform);
            }
        }
    };
}

macro_rules! get_smoothed_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter(
            $self
                .channel_params
                .iter()
                .map(|channel| channel.$param.get()),
        )
    };
}
