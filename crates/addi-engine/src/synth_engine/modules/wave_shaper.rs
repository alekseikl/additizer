use std::array;

use itertools::izip;

use crate::{
    synth_engine::db_to_gain_fast,
    synth_engine::{
        Sample, SmoothedSampleParams, StereoSample,
        buffer::{Buffer, VoicesLayout, zero_buffer},
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, MixedSlots, ModuleId,
            NUM_CHANNELS, ProcessContext, RouterFactory, SamplesOutput, VoiceTarget,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
    },
};

mod config;
mod link;
pub mod stub;

pub use config::{ShaperType, WaveShaperConfig};
pub use link::{UiEvent, WaveShaperAudioEnd, WaveShaperLinks, WaveShaperUiEnd};

struct Params {
    shaper_type: ShaperType,
}

impl ShaperType {
    pub fn apply(self, input: Sample, gain: Sample, clipping_gain: Sample) -> Sample {
        match self {
            Self::HardClip => (input * gain).clamp(-clipping_gain, clipping_gain),
            Self::Sigmoid => clipping_gain * (input * gain / clipping_gain).tanh(),
        }
    }
}

impl Params {
    fn from_config(c: &config::WaveShaperConfig) -> Self {
        Self {
            shaper_type: c.shaper_type,
        }
    }
}

struct ChannelParams {
    distortion: SmoothedSample,
    clipping_level: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &WaveShaperConfig, channel_idx: usize) -> Self {
        Self {
            distortion: c.distortion[channel_idx].into(),
            clipping_level: c.clipping_level[channel_idx].into(),
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.distortion.advance(smooth_params, samples);
        self.clipping_level.advance(smooth_params, samples);
    }
}

pub struct Inputs {
    audio: Option<usize>,
    distortion: MixedSlots,
    clipping_level: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            audio: None,
            distortion: MixedSlots::new(Input::Distortion),
            clipping_level: MixedSlots::new(Input::ClippingLevel),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input {
                InputSlots::Direct { input_type, slot } => {
                    if matches!(input_type, Input::Audio) {
                        result.audio = Some(*slot);
                    }
                }
                InputSlots::Mixed(input) => match input.input_type {
                    Input::Distortion => result.distortion = input.clone(),
                    Input::ClippingLevel => result.clipping_level = input.clone(),
                    _ => (),
                },
                InputSlots::Spectral { .. } => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Distortion => self.distortion.update_amount(src_slot, amount),
            Input::ClippingLevel => self.clipping_level.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

struct Buffers {
    distortion_mod_input: Buffer,
    clipping_level_mod_input: Buffer,
}

pub struct WaveShaper<L: WaveShaperLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl<L: WaveShaperLinks> WaveShaper<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&WaveShaperConfig {
            id,
            ..WaveShaperConfig::default()
        })
    }

    pub fn from_config(config: &config::WaveShaperConfig) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers {
                distortion_mod_input: zero_buffer(),
                clipping_level_mod_input: zero_buffer(),
            },
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
        }
    }

    pub fn get_config(&self) -> WaveShaperConfig {
        WaveShaperConfig {
            id: self.id,
            shaper_type: self.params.shaper_type,
            distortion: get_smoothed_param!(self, distortion),
            clipping_level: get_smoothed_param!(self, clipping_level),
        }
    }

    set_mono_param!(set_shaper_type, shaper_type, ShaperType);

    set_smoothed_param!(set_distortion, distortion);
    set_smoothed_param!(set_clipping_level, clipping_level);

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<AudioRouterType, L::EngineEnd>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let channel = &mut self.channel_params[target.channel_idx];

        router.param(
            &inputs.clipping_level,
            &channel.clipping_level,
            &mut self.buffers.clipping_level_mod_input,
        );
        router.param(
            &inputs.distortion,
            &channel.distortion,
            &mut self.buffers.distortion_mod_input,
        );

        for (out, input, clipping_level_mod, distortion_mod) in izip!(
            voice_output.output().iter_mut(),
            router.direct(inputs.audio),
            &self.buffers.clipping_level_mod_input,
            &self.buffers.distortion_mod_input
        ) {
            let clipping_gain = db_to_gain_fast(clipping_level_mod.min(24.0));
            let gain = db_to_gain_fast(distortion_mod.clamp(0.0, 48.0));

            *out = self.params.shaper_type.apply(*input, gain, clipping_gain);
        }
    }
    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.audio(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            })
            .for_channels(|rf, channel_idx| {
                self.channel_params[channel_idx]
                    .advance_smoothers(&rf.params().smooth_params, rf.params().samples);
            });
    }
}

impl<L: WaveShaperLinks> SynthModule for WaveShaper<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::direct_audio(Input::Audio),
            InputMeta::control(Input::ClippingLevel),
            InputMeta::control(Input::Distortion),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Audio
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots]) {
        self.inputs = Inputs::from_slots(inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Distortion => self.set_distortion(value),
                    Input::ClippingLevel => self.set_clipping_level(value),
                    _ => (),
                },
                UiEvent::ShaperType(shaper_type) => self.set_shaper_type(shaper_type),
            }
        }
    }
}
