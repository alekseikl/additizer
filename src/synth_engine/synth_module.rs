use std::{any::Any, sync::Arc};

use parking_lot::Mutex;

use crate::synth_engine::{
    ModuleInput,
    buffer::{Buffer, SpectralBuffer, ZEROES_BUFFER, ZEROES_SPECTRAL_BUFFER},
    routing::{DataType, Input, ModuleId, ModuleType, Router, VoiceEvent},
    smooth::{SmoothedSample, SmoothedSampleParams},
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

    fn inputs(&self) -> &'static [ModInput];
    fn output(&self) -> DataType;

    fn handle_events(&mut self, events: &[VoiceEvent]) {}
    fn handle_ui_events(&mut self);
    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {}

    fn process(&mut self, params: &ProcessParams, router: &mut dyn Router);

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

    pub fn sample_rate(&self) -> Sample {
        self.factory.process_params.sample_rate
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

    pub fn buff_param(&mut self, input: Input, param: &mut SmoothedSample, buff: &mut Buffer) {
        let params = &self.factory.process_params;
        let buff = &mut buff[..params.samples];

        if param.check_needs_smoothing(&params.smooth_params) {
            param.smoothed_buff(buff, &params.smooth_params);
        } else {
            buff.fill(param.get());
        }

        if self.factory.router.add_input_to(
            ModuleInput::new(input, self.factory.module_id),
            self.voice_idx,
            self.channel_idx,
            buff,
        ) && params.needs_update_ui
            && self.voice_seq_idx == 0
        {
            self.factory.router.update_modulated_input(
                self.factory.module_id,
                input,
                self.channel_idx,
                buff[0],
            );
        }
    }

    pub fn update_output(&mut self, buff: &Buffer) {
        self.factory
            .router
            .update_output(self.factory.module_id, self.channel_idx, buff[0]);
    }

    pub fn spectral(&self, input: Input, current: bool) -> &SpectralBuffer {
        self.factory
            .router
            .get_spectral_input(
                ModuleInput::new(input, self.factory.module_id),
                current,
                self.voice_idx,
                self.channel_idx,
            )
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER)
    }

    pub fn scalar(&mut self, input: Input, param: Sample, current: bool) -> Sample {
        let value = self.factory.router.get_scalar_input(
            ModuleInput::new(input, self.factory.module_id),
            current,
            self.voice_idx,
            self.channel_idx,
        );

        if let Some(value) = value {
            let value = value + param;

            if self.factory.process_params.needs_update_ui && self.voice_seq_idx == 0 {
                self.factory.router.update_modulated_input(
                    self.factory.module_id,
                    input,
                    self.channel_idx,
                    value,
                );
            }

            value
        } else {
            param
        }
    }
}

pub type ModuleConfigBox<T> = Arc<Mutex<T>>;

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
            for (channel, $param) in self.channels.iter_mut().zip($param.iter()) {
                channel.params.$param = $transform;
            }
        }
    };
}

macro_rules! get_stereo_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channels.iter().map(|channel| channel.params.$param))
    };
}

macro_rules! set_stereo_param2 {
    ($fn_name:ident, $param:ident) => {
        set_stereo_param2!($fn_name, $param, *$param);
    };
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channel_params.iter_mut().zip($param.iter()) {
                channel.$param = $transform;
            }
        }
    };
}

macro_rules! get_stereo_param2 {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channel_params.iter().map(|channel| channel.$param))
    };
}

macro_rules! set_smoothed_param2 {
    ($fn_name:ident, $param:ident) => {
        set_smoothed_param2!($fn_name, $param, *$param);
    };
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channel_params.iter_mut().zip($param.iter()) {
                channel.$param.set($transform);
            }
        }
    };
}

macro_rules! get_smoothed_param2 {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter(
            $self
                .channel_params
                .iter()
                .map(|channel| channel.$param.get()),
        )
    };
}

macro_rules! load_module_config {
    ($self:ident) => {{
        let cfg = $self.config.lock();

        for (channel, cfg_channel) in $self.channels.iter_mut().zip(cfg.channels.iter()) {
            channel.params = cfg_channel.clone();
        }

        $self.params = cfg.params.clone();
    }};
}
