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
    pub needs_audio_rate: bool,
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
    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {}

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

pub trait ModuleToUiBridge {
    fn update_modulated_input(&mut self, input: Input, channel_idx: usize, value: Sample);
    fn update_output(&mut self, channel_idx: usize, value: Sample);
}

pub struct MockToUiBridge;

impl ModuleToUiBridge for MockToUiBridge {
    fn update_modulated_input(&mut self, _input: Input, _channel_idx: usize, _value: Sample) {}
    fn update_output(&mut self, _channel_idx: usize, _value: Sample) {}
}

pub struct VoiceRouter<'a, Bridge: ModuleToUiBridge> {
    pub router: &'a dyn Router,
    pub ui_bridge: &'a mut Bridge,
    pub module_id: ModuleId,
    pub samples: usize,
    pub sample_rate: Sample,
    pub smooth_params: SmoothedSampleParams,
    pub voice_idx: usize,
    pub channel_idx: usize,
}

impl<'a, Bridge: ModuleToUiBridge> VoiceRouter<'a, Bridge> {
    pub fn new(
        router: &'a dyn Router,
        module_id: ModuleId,
        channel_idx: usize,
        voice_idx: usize,
        params: &ProcessParams,
        ui_bridge: &'a mut Bridge,
    ) -> Self {
        Self {
            router,
            module_id,
            samples: params.samples,
            sample_rate: params.sample_rate,
            smooth_params: params.smooth_params,
            voice_idx,
            channel_idx,
            ui_bridge,
        }
    }

    pub fn buffer_opt(&'a self, input: Input, buff: &'a mut Buffer) -> Option<&'a Buffer> {
        self.router.get_input(
            ModuleInput::new(input, self.module_id),
            self.samples,
            self.voice_idx,
            self.channel_idx,
            buff,
        )
    }

    pub fn buffer(&'a self, input: Input, buff: &'a mut Buffer) -> &'a Buffer {
        self.buffer_opt(input, buff).unwrap_or(&ZEROES_BUFFER)
    }

    pub fn buff_param(&mut self, input: Input, param: &mut SmoothedSample, buff: &mut Buffer) {
        let buff = &mut buff[..self.samples];

        if param.check_needs_smoothing(&self.smooth_params) {
            param.smoothed_buff(buff, &self.smooth_params);
        } else {
            buff.fill(param.get());
        }

        if self.router.add_input_to(
            ModuleInput::new(input, self.module_id),
            self.voice_idx,
            self.channel_idx,
            buff,
        ) {
            self.ui_bridge
                .update_modulated_input(input, self.channel_idx, buff[0]);
        }
    }

    pub fn update_output(&mut self, buff: &Buffer) {
        self.ui_bridge.update_output(self.channel_idx, buff[0]);
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

        #[allow(dead_code)]
        pub fn downcast_mut_unwrap(module: Option<&mut dyn SynthModule>) -> &mut Self {
            Self::downcast_mut(module.unwrap()).unwrap()
        }
    };
}

macro_rules! set_mono_param {
    ($fn_name:ident, $param:ident, $type:ty) => {
        pub fn $fn_name(&mut self, $param: $type) {
            self.params.$param = $param;
            self.config.lock().params.$param = self.params.$param;
        }
    };
    ($fn_name:ident, $param:ident, $type:ty, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: $type) {
            self.params.$param = $transform;
            self.config.lock().params.$param = self.params.$param;
        }
    };
}

macro_rules! set_mono_param_inline {
    ($self:ident, $param:ident, $value:expr) => {{
        $self.params.$param = $value;
        $self.config.lock().params.$param = $self.params.$param;
    }};
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

            let mut cfg = self.config.lock();

            for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                config_channel.$param = channel.params.$param;
            }
        }
    };
}

macro_rules! get_stereo_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channels.iter().map(|channel| channel.params.$param))
    };
}

macro_rules! set_smoothed_param {
    ($fn_name:ident, $param:ident) => {
        set_smoothed_param!($fn_name, $param, *$param);
    };
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channels.iter_mut().zip($param.iter()) {
                channel.params.$param.set($transform);
            }

            let mut cfg = self.config.lock();

            for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                config_channel.$param.set(channel.params.$param.get());
            }
        }
    };
}

macro_rules! set_param_inline {
    ($self:ident, $param:ident, $value:expr) => {{
        for (channel, $param) in $self.channels.iter_mut().zip($value.iter()) {
            channel.params.$param.set(*$param);
        }

        let mut cfg = $self.config.lock();

        for (config_channel, channel) in cfg.channels.iter_mut().zip($self.channels.iter()) {
            config_channel.$param.set(channel.params.$param.get());
        }
    }};
}

macro_rules! set_stereo_param_inline {
    ($self:ident, $param:ident, $value:expr) => {{
        for (channel, val) in $self.channels.iter_mut().zip($value.iter()) {
            channel.params.$param = *val;
        }

        let mut cfg = $self.config.lock();

        for (config_channel, channel) in cfg.channels.iter_mut().zip($self.channels.iter()) {
            config_channel.$param = channel.params.$param;
        }
    }};
}

macro_rules! get_smoothed_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter(
            $self
                .channels
                .iter()
                .map(|channel| channel.params.$param.get()),
        )
    };
}

macro_rules! load_module_config {
    ($self:ident) => {{
        let cfg = $self.config.lock();

        if let Some(label) = cfg.label.as_ref() {
            $self.label = label.clone();
        }

        for (channel, cfg_channel) in $self.channels.iter_mut().zip(cfg.channels.iter()) {
            channel.params = cfg_channel.clone();
        }

        $self.params = cfg.params.clone();
    }};
}

macro_rules! load_module_config_no_params {
    ($self:ident) => {{
        let cfg = $self.config.lock();

        if let Some(label) = cfg.label.as_ref() {
            $self.label = label.clone();
        }

        for (channel, cfg_channel) in $self.channels.iter_mut().zip(cfg.channels.iter()) {
            channel.params = cfg_channel.clone();
        }
    }};
}
