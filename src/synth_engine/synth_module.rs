use enum_dispatch::enum_dispatch;

use crate::synth_engine::{
    StereoSample,
    routing::{
        DataType, Input, InputMeta, InputSlots, ModuleId, ProcessContext, SpectralInputSlot,
        VoiceEvent,
    },
    voices_handler::DecayingVoice,
};

#[enum_dispatch]
#[auto_impl::auto_impl(Box)]
#[allow(unused_variables)]
pub(super) trait SynthModule: Send {
    fn id(&self) -> ModuleId;

    fn inputs(&self) -> &'static [InputMeta];
    fn output_type(&self) -> DataType;

    fn set_output_slot(&mut self, slot: usize);
    fn output_slot(&self) -> usize;

    fn set_input_slots(&mut self, inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]);

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample);

    fn process_events(&mut self, events: &[VoiceEvent]) {}
    fn process_ui_events(&mut self);
    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {}

    fn process(&mut self, ctx: &mut ProcessContext);
}

#[enum_dispatch]
#[auto_impl::auto_impl(Box)]
pub trait ModuleUiBridge: Send {
    fn update(&mut self);
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
