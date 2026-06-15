use std::array;

use itertools::izip;
use nih_plug::util::db_to_gain_fast;

mod config;
mod link;
mod ui_bridge;

pub use config::{ShaperType, WaveShaperConfig};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::WaveShaperUiBridge;

use crate::synth_engine::{
    StereoSample,
    buffer::{Buffer, VoicesLayout, zero_buffer},
    routing::{
        AudioRouterType, DataType, Input, InputSlots, ModuleId, ModuleType, NUM_CHANNELS,
        ProcessContext, SamplesOutput, SpectralInputSlot, VoiceRouter,
    },
    smooth::SmoothedSample,
    synth_module::{ModInput, SynthModule},
};

struct Params {
    shaper_type: ShaperType,
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
}

pub struct Inputs {
    audio: Option<usize>,
    distortion: InputSlots,
    clipping_level: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            audio: None,
            distortion: InputSlots::empty(Input::Distortion),
            clipping_level: InputSlots::empty(Input::ClippingLevel),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Audio => result.audio = input.slots.first().map(|s| s.src_slot),
                Input::Distortion => result.distortion = input.clone(),
                Input::ClippingLevel => result.clipping_level = input.clone(),
                _ => (),
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

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, AudioRouterType>;

struct Buffers {
    distortion_mod_input: Buffer,
    clipping_level_mod_input: Buffer,
}

pub struct WaveShaper {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl WaveShaper {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&WaveShaperConfig {
            id,
            ..WaveShaperConfig::default()
        })
    }

    pub fn from_config(config: &config::WaveShaperConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

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
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            output_slot: 0,
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
        output: &mut VoicesLayout<SamplesOutput>,
        mut router: Router<'_, '_, '_>,
    ) {
        let channel_idx = router.channel_idx();
        let voice_idx = router.voice_idx();
        let inputs = &self.inputs;
        let channel = &mut self.channel_params[channel_idx];
        let output = output[channel_idx][voice_idx].output(router.samples());

        router.buff_param(
            &inputs.clipping_level,
            &mut channel.clipping_level,
            &mut self.buffers.clipping_level_mod_input,
        );
        router.buff_param(
            &inputs.distortion,
            &mut channel.distortion,
            &mut self.buffers.distortion_mod_input,
        );

        for (out, input, clipping_level_mod, distortion_mod) in izip!(
            output.iter_mut(),
            router.buff(inputs.audio),
            &self.buffers.clipping_level_mod_input,
            &self.buffers.distortion_mod_input
        ) {
            let clipping_gain = db_to_gain_fast(clipping_level_mod.min(24.0));
            let gain = db_to_gain_fast(distortion_mod.clamp(0.0, 48.0));

            match self.params.shaper_type {
                ShaperType::HardClip => {
                    *out = (input * gain).clamp(-clipping_gain, clipping_gain);
                }
                ShaperType::Sigmoid => *out = clipping_gain * (input * gain / clipping_gain).tanh(),
            }
        }
    }
}

impl SynthModule for WaveShaper {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::WaveShaper
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::audio(Input::Audio),
            ModInput::audio(Input::ClippingLevel),
            ModInput::audio(Input::Distortion),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Audio
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_slots(
        &mut self,
        inputs: &[InputSlots],
        spectral_inputs: &[SpectralInputSlot],
        output_slot: usize,
    ) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
        self.output_slot = output_slot;
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

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_audio(self.id, self.output_slot, |router, output| {
            let num_active_voices = router.params().active_voices.len();

            for channel_idx in 0..NUM_CHANNELS {
                for seq_idx in 0..num_active_voices {
                    let voice_idx = router.params().active_voices[seq_idx];

                    self.process_voice(output, router.for_voice(channel_idx, voice_idx, seq_idx));
                }
            }
        });
    }
}
