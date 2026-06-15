use enum_dispatch::enum_dispatch;

use crate::synth_engine::{
    StereoSample,
    routing::{
        DataType, Input, InputSlots, ModuleId, ProcessContext, SpectralInputSlot,
        VoiceEvent,
    },
    voices_handler::DecayingVoice,
};

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

#[enum_dispatch]
#[allow(unused_variables)]
pub(super) trait SynthModule: Send {
    fn id(&self) -> ModuleId;

    fn inputs(&self) -> &'static [ModInput];
    fn output(&self) -> DataType;

    fn output_slot(&self) -> usize;

    fn set_slots(
        &mut self,
        inputs: &[InputSlots],
        spectral_inputs: &[SpectralInputSlot],
        output_slot: usize,
    );

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample);

    fn process_events(&mut self, events: &[VoiceEvent]) {}
    fn process_ui_events(&mut self);
    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {}

    fn process(&mut self, ctx: &mut ProcessContext);
}

impl<T: SynthModule> SynthModule for Box<T> {
    fn id(&self) -> ModuleId {
        (**self).id()
    }

    fn inputs(&self) -> &'static [ModInput] {
        (**self).inputs()
    }

    fn output(&self) -> DataType {
        (**self).output()
    }

    fn output_slot(&self) -> usize {
        (**self).output_slot()
    }

    fn set_slots(
        &mut self,
        inputs: &[InputSlots],
        spectral_inputs: &[SpectralInputSlot],
        output_slot: usize,
    ) {
        (**self).set_slots(inputs, spectral_inputs, output_slot)
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        (**self).update_input_amount(input_type, src_slot, amount)
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        (**self).process_events(events)
    }

    fn process_ui_events(&mut self) {
        (**self).process_ui_events()
    }

    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {
        (**self).poll_decaying_voices(decaying_voices)
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        (**self).process(ctx)
    }
}

#[enum_dispatch]
pub trait ModuleUiBridge: Send {
    fn update(&mut self);
}

impl<T: ModuleUiBridge> ModuleUiBridge for Box<T> {
    fn update(&mut self) {
        (**self).update()
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
