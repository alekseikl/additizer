use std::array;

use nih_plug::util::db_to_gain_fast;

mod config;
mod link;
mod ui_bridge;

pub use config::{MAX_INPUTS, MixerConfig};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::MixerUiBridge;

use crate::synth_engine::{
    StereoSample,
    buffer::{Buffer, VoicesLayout, copy_or_add_to_buffer, zero_buffer},
    routing::{
        AudioRouterType, DataType, Input, InputSlots, ModuleId, ModuleType, NUM_CHANNELS,
        ProcessContext, SamplesOutput, SpectralInputSlot, VoiceRouter, VolumeType,
    },
    smooth::SmoothedSample,
    synth_module::{ModInput, SynthModule},
    types::Sample,
};

const MAX_VOLUME: Sample = 24.0; // dB

struct InputChannelParams {
    level: SmoothedSample,
    gain: SmoothedSample,
}

struct ChannelParams {
    input_params: [InputChannelParams; MAX_INPUTS as usize],
    output_level: SmoothedSample,
    output_gain: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &config::MixerConfig, channel_idx: usize) -> Self {
        Self {
            input_params: c.inputs.map(|input| InputChannelParams {
                level: input.level[channel_idx].into(),
                gain: input.gain[channel_idx].into(),
            }),
            output_level: c.output_level[channel_idx].into(),
            output_gain: c.output_gain[channel_idx].into(),
        }
    }
}

struct InputParams {
    volume_type: VolumeType,
}

struct Params {
    num_inputs: u8,
    inputs: [InputParams; MAX_INPUTS as usize],
    output_volume_type: VolumeType,
}

impl Params {
    fn from_config(c: &config::MixerConfig) -> Self {
        Self {
            num_inputs: c.num_inputs.clamp(1, MAX_INPUTS),
            inputs: c.inputs.map(|input| InputParams {
                volume_type: input.volume_type,
            }),
            output_volume_type: c.output_volume_type,
        }
    }
}

pub struct Inputs {
    gain: InputSlots,
    level: InputSlots,
    audio_mix: [InputSlots; MAX_INPUTS as usize],
    gain_mix: [InputSlots; MAX_INPUTS as usize],
    level_mix: [InputSlots; MAX_INPUTS as usize],
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            gain: InputSlots::empty(Input::Gain),
            level: InputSlots::empty(Input::Level),
            audio_mix: array::from_fn(|idx| InputSlots::empty(Input::AudioMix(idx as u8))),
            gain_mix: array::from_fn(|idx| InputSlots::empty(Input::GainMix(idx as u8))),
            level_mix: array::from_fn(|idx| InputSlots::empty(Input::LevelMix(idx as u8))),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Gain => result.gain = input.clone(),
                Input::Level => result.level = input.clone(),
                Input::GainMix(idx) if idx < MAX_INPUTS => {
                    result.gain_mix[idx as usize] = input.clone();
                }
                Input::LevelMix(idx) if idx < MAX_INPUTS => {
                    result.level_mix[idx as usize] = input.clone();
                }
                Input::AudioMix(idx) if idx < MAX_INPUTS => {
                    result.audio_mix[idx as usize] = input.clone();
                }
                _ => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Gain => self.gain.update_amount(src_slot, amount),
            Input::Level => self.level.update_amount(src_slot, amount),
            Input::GainMix(idx) if idx < MAX_INPUTS => {
                self.gain_mix[idx as usize].update_amount(src_slot, amount);
            }
            Input::LevelMix(idx) if idx < MAX_INPUTS => {
                self.level_mix[idx as usize].update_amount(src_slot, amount);
            }
            Input::AudioMix(idx) if idx < MAX_INPUTS => {
                self.audio_mix[idx as usize].update_amount(src_slot, amount);
            }
            _ => (),
        }
    }
}

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, AudioRouterType>;

struct Buffers {
    level_mod: Buffer,
}

impl Default for Buffers {
    fn default() -> Self {
        Self {
            level_mod: zero_buffer(),
        }
    }
}

pub struct Mixer {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl Mixer {
    pub const MAX_INPUTS: u8 = MAX_INPUTS;

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&MixerConfig {
            id,
            ..MixerConfig::default()
        })
    }

    pub fn from_config(config: &config::MixerConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers::default(),
            audio_end,
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            output_slot: 0,
        }
    }

    pub fn get_config(&self) -> MixerConfig {
        MixerConfig {
            id: self.id,
            num_inputs: self.params.num_inputs,
            inputs: array::from_fn(|input_idx| config::InputConfig {
                volume_type: self.params.inputs[input_idx].volume_type,
                level: StereoSample::from_iter(
                    self.channel_params
                        .iter()
                        .map(|channel| channel.input_params[input_idx].level.get()),
                ),
                gain: StereoSample::from_iter(
                    self.channel_params
                        .iter()
                        .map(|channel| channel.input_params[input_idx].gain.get()),
                ),
            }),
            output_volume_type: self.params.output_volume_type,
            output_level: get_smoothed_param!(self, output_level),
            output_gain: get_smoothed_param!(self, output_gain),
        }
    }

    set_mono_param!(
        set_num_inputs,
        num_inputs,
        u8,
        num_inputs.clamp(1, MAX_INPUTS)
    );

    set_mono_param!(set_output_volume_type, output_volume_type, VolumeType);

    set_smoothed_param!(set_output_level, output_level);
    set_smoothed_param!(set_output_gain, output_gain);

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;
        self.params.inputs[input_idx].volume_type = volume_type;
    }

    pub fn set_input_level(&mut self, input_idx: u8, level: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, level) in self.channel_params.iter_mut().zip(level.iter()) {
            channel.input_params[input_idx].level.set(*level);
        }
    }

    pub fn set_input_gain(&mut self, input_idx: u8, gain: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, gain) in self.channel_params.iter_mut().zip(gain.iter()) {
            channel.input_params[input_idx].gain.set(*gain);
        }
    }

    #[inline(always)]
    fn to_gain(dbs: Sample) -> Sample {
        db_to_gain_fast(dbs.min(MAX_VOLUME))
    }

    #[inline(always)]
    fn mix_input(
        output: &mut [Sample],
        input: &[Sample],
        gain_mod: impl Iterator<Item = Sample>,
        input_idx: u8,
    ) {
        let input = input
            .iter()
            .zip(gain_mod)
            .map(|(sample, gain_mod)| sample * gain_mod);

        copy_or_add_to_buffer(input_idx == 0, output, input);
    }

    #[inline(always)]
    fn modulate_output(output: &mut [Sample], gain_mod: impl Iterator<Item = Sample>) {
        for (out, gain_mod) in output.iter_mut().zip(gain_mod) {
            *out *= gain_mod;
        }
    }

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

        for input_idx in 0..self.params.num_inputs {
            let input_params = &self.params.inputs[input_idx as usize];
            let input_channel = &mut channel.input_params[input_idx as usize];

            match input_params.volume_type {
                VolumeType::Db => {
                    router.buff_param(
                        &inputs.level_mix[input_idx as usize],
                        &mut input_channel.level,
                        &mut self.buffers.level_mod,
                    );
                    let gain_mod = self
                        .buffers
                        .level_mod
                        .iter()
                        .map(|level| Self::to_gain(*level));

                    Self::mix_input(
                        output,
                        router.buff(inputs.audio_mix[input_idx as usize].first_slot()),
                        gain_mod,
                        input_idx,
                    );
                }
                VolumeType::Gain => {
                    router.buff_param(
                        &inputs.gain_mix[input_idx as usize],
                        &mut input_channel.gain,
                        &mut self.buffers.level_mod,
                    );
                    let gain_mod = self.buffers.level_mod.iter().copied();

                    Self::mix_input(
                        output,
                        router.buff(inputs.audio_mix[input_idx as usize].first_slot()),
                        gain_mod,
                        input_idx,
                    );
                }
            }
        }

        match self.params.output_volume_type {
            VolumeType::Db => {
                router.buff_param(
                    &inputs.level,
                    &mut channel.output_level,
                    &mut self.buffers.level_mod,
                );
                let gain_mod = self
                    .buffers
                    .level_mod
                    .iter()
                    .map(|level| Self::to_gain(*level));

                Self::modulate_output(output, gain_mod);
            }
            VolumeType::Gain => {
                router.buff_param(
                    &inputs.gain,
                    &mut channel.output_gain,
                    &mut self.buffers.level_mod,
                );
                let gain_mod = self.buffers.level_mod.iter().copied();

                Self::modulate_output(output, gain_mod);
            }
        }
    }
}

impl SynthModule for Mixer {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Mixer
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::audio(Input::Gain),
            ModInput::audio(Input::Level),
            ModInput::audio(Input::AudioMix(0)),
            ModInput::audio(Input::GainMix(0)),
            ModInput::audio(Input::LevelMix(0)),
            ModInput::audio(Input::AudioMix(1)),
            ModInput::audio(Input::GainMix(1)),
            ModInput::audio(Input::LevelMix(1)),
            ModInput::audio(Input::AudioMix(2)),
            ModInput::audio(Input::GainMix(2)),
            ModInput::audio(Input::LevelMix(2)),
            ModInput::audio(Input::AudioMix(3)),
            ModInput::audio(Input::GainMix(3)),
            ModInput::audio(Input::LevelMix(3)),
            ModInput::audio(Input::AudioMix(4)),
            ModInput::audio(Input::GainMix(4)),
            ModInput::audio(Input::LevelMix(4)),
            ModInput::audio(Input::AudioMix(5)),
            ModInput::audio(Input::GainMix(5)),
            ModInput::audio(Input::LevelMix(5)),
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

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Gain => self.set_output_gain(value),
                    Input::Level => self.set_output_level(value),
                    Input::GainMix(idx) => self.set_input_gain(idx, value),
                    Input::LevelMix(idx) => self.set_input_level(idx, value),
                    _ => (),
                },
                UiEvent::NumInputs(num_inputs) => self.set_num_inputs(num_inputs),
                UiEvent::InputVolumeType {
                    input_idx,
                    volume_type,
                } => self.set_volume_type(input_idx, volume_type),
                UiEvent::OutputVolumeType(volume_type) => self.set_output_volume_type(volume_type),
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
