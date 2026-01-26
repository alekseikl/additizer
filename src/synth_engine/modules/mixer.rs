use std::any::Any;
use std::array;

use nih_plug::util::db_to_gain_fast;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule, VolumeType,
    buffer::{Buffer, copy_or_add_buffer, zero_buffer},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
    synth_module::{InputInfo, ModuleConfigBox, ProcessParams, VoiceRouter},
};

const MAX_INPUTS: usize = 6;
const MAX_VOLUME: Sample = 24.0; // dB

#[derive(Clone, Serialize, Deserialize)]
pub struct InputChannelParams {
    gain: Sample,  // 0.0-1.0
    level: Sample, // dB
}

impl Default for InputChannelParams {
    fn default() -> Self {
        Self {
            gain: 1.0,
            level: 0.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    input_params: [InputChannelParams; MAX_INPUTS],
    output_level: Sample,
    output_gain: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            input_params: Default::default(),
            output_level: 0.0,
            output_gain: 1.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    num_inputs: usize,
    input_volume_types: [VolumeType; MAX_INPUTS],
    output_volume_type: VolumeType,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            num_inputs: 2,
            input_volume_types: Default::default(),
            output_volume_type: VolumeType::Gain,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct MixerConfig {
    label: Option<String>,
    params: Params,
    channels: [ChannelParams; NUM_CHANNELS],
}

pub struct MixerUIData {
    pub label: String,
    pub num_inputs: usize,
    pub input_volume_types: [VolumeType; MAX_INPUTS],
    pub input_levels: [StereoSample; MAX_INPUTS],
    pub input_gains: [StereoSample; MAX_INPUTS],
    pub output_volume_type: VolumeType,
    pub output_level: StereoSample,
    pub output_gain: StereoSample,
}

struct Voice {
    output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            output: zero_buffer(),
        }
    }
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

struct Buffers {
    input: Buffer,
    level_mod: Buffer,
}

impl Default for Buffers {
    fn default() -> Self {
        Self {
            input: zero_buffer(),
            level_mod: zero_buffer(),
        }
    }
}

pub struct Mixer {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<MixerConfig>,
    params: Params,
    buffers: Buffers,
    channels: [Channel; NUM_CHANNELS],
}

impl Mixer {
    pub const MAX_INPUTS: usize = MAX_INPUTS;

    pub fn new(id: ModuleId, config: ModuleConfigBox<MixerConfig>) -> Self {
        let mut mixer = Self {
            id,
            label: format!("Mixer {id}"),
            config,
            params: Params::default(),
            buffers: Buffers::default(),
            channels: Default::default(),
        };

        load_module_config!(mixer);
        mixer
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> MixerUIData {
        MixerUIData {
            label: self.label.clone(),
            num_inputs: self.params.num_inputs,
            input_volume_types: self.params.input_volume_types,
            input_gains: array::from_fn(|idx| {
                self.channels
                    .iter()
                    .map(|channel| channel.params.input_params[idx].gain)
                    .collect()
            }),
            input_levels: array::from_fn(|idx| {
                self.channels
                    .iter()
                    .map(|channel| channel.params.input_params[idx].level)
                    .collect()
            }),
            output_volume_type: self.params.output_volume_type,
            output_gain: get_stereo_param!(self, output_gain),
            output_level: get_stereo_param!(self, output_level),
        }
    }

    set_mono_param!(
        set_num_inputs,
        num_inputs,
        usize,
        num_inputs.clamp(1, MAX_INPUTS)
    );

    set_mono_param!(set_output_volume_type, output_volume_type, VolumeType);

    set_stereo_param!(set_output_level, output_level);
    set_stereo_param!(set_output_gain, output_gain);

    pub fn set_volume_type(&mut self, input_idx: usize, volume_type: VolumeType) {
        self.params.input_volume_types[input_idx] = volume_type;
        self.config.lock().params.input_volume_types[input_idx] = volume_type;
    }

    pub fn set_input_level(&mut self, input_idx: usize, level: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS);

        for (channel, level) in self.channels.iter_mut().zip(level.iter()) {
            channel.params.input_params[input_idx].level = *level;
        }

        let mut cfg = self.config.lock();

        for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            config_channel.input_params[input_idx].level =
                channel.params.input_params[input_idx].level;
        }
    }

    pub fn set_input_gain(&mut self, input_idx: usize, gain: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS);

        for (channel, gain) in self.channels.iter_mut().zip(gain.iter()) {
            channel.params.input_params[input_idx].gain = *gain;
        }

        let mut cfg = self.config.lock();

        for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            config_channel.input_params[input_idx].gain =
                channel.params.input_params[input_idx].gain;
        }
    }

    #[inline(always)]
    fn to_gain(dbs: Sample) -> Sample {
        db_to_gain_fast(dbs.min(MAX_VOLUME))
    }

    #[inline(always)]
    fn mix_input(
        output: &mut Buffer,
        input: &Buffer,
        gain_mod: impl Iterator<Item = Sample>,
        input_idx: usize,
        samples: usize,
    ) {
        let input = input
            .iter()
            .zip(gain_mod)
            .map(|(sample, gain_mod)| sample * gain_mod);

        copy_or_add_buffer(input_idx == 0, output, input.take(samples));
    }

    fn modulate_output(
        output: &mut Buffer,
        gain_mod: impl Iterator<Item = Sample>,
        samples: usize,
    ) {
        for (out, gain_mod) in output.iter_mut().zip(gain_mod).take(samples) {
            *out *= gain_mod;
        }
    }

    fn process_voice(
        params: &Params,
        channel: &ChannelParams,
        buffers: &mut Buffers,
        voice: &mut Voice,
        router: &VoiceRouter,
    ) {
        let samples = router.samples;

        for input_idx in 0..params.num_inputs {
            let input = router.buffer(Input::AudioMix(input_idx), &mut buffers.input);

            match params.input_volume_types[input_idx] {
                VolumeType::Db => {
                    let channel_level = channel.input_params[input_idx].level;
                    let level_mod =
                        router.buffer(Input::LevelMix(input_idx), &mut buffers.level_mod);
                    let gain_mod = level_mod
                        .iter()
                        .map(|level_mod| Self::to_gain(channel_level + level_mod));

                    Self::mix_input(&mut voice.output, input, gain_mod, input_idx, samples);
                }
                VolumeType::Gain => {
                    let channel_gain = channel.input_params[input_idx].gain;
                    let gain_mod = router.buffer(Input::GainMix(input_idx), &mut buffers.level_mod);
                    let gain_mod = gain_mod.iter().map(|gain_mod| channel_gain + gain_mod);

                    Self::mix_input(&mut voice.output, input, gain_mod, input_idx, samples);
                }
            }
        }

        match params.output_volume_type {
            VolumeType::Db => {
                let output_level = channel.output_level;
                let level_mod = router.buffer(Input::Level, &mut buffers.level_mod);
                let gain_mod = level_mod
                    .iter()
                    .map(|level_mod| Self::to_gain(output_level + level_mod));

                Self::modulate_output(&mut voice.output, gain_mod, samples);
            }
            VolumeType::Gain => {
                let output_gain = channel.output_gain;
                let gain_mod = router.buffer(Input::Gain, &mut buffers.level_mod);
                let gain_mod = gain_mod.iter().map(|gain_mod| output_gain + gain_mod);

                Self::modulate_output(&mut voice.output, gain_mod, samples);
            }
        }
    }
}

impl SynthModule for Mixer {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn set_label(&mut self, label: String) {
        self.label = label.clone();
        self.config.lock().label = Some(label);
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Mixer
    }

    fn inputs(&self) -> &'static [InputInfo] {
        static INPUTS: &[InputInfo] = &[
            InputInfo::buffer(Input::Gain),
            InputInfo::buffer(Input::Level),
            InputInfo::buffer(Input::AudioMix(0)),
            InputInfo::buffer(Input::GainMix(0)),
            InputInfo::buffer(Input::LevelMix(0)),
            InputInfo::buffer(Input::AudioMix(1)),
            InputInfo::buffer(Input::GainMix(1)),
            InputInfo::buffer(Input::LevelMix(1)),
            InputInfo::buffer(Input::AudioMix(2)),
            InputInfo::buffer(Input::GainMix(2)),
            InputInfo::buffer(Input::LevelMix(2)),
            InputInfo::buffer(Input::AudioMix(3)),
            InputInfo::buffer(Input::GainMix(3)),
            InputInfo::buffer(Input::LevelMix(3)),
            InputInfo::buffer(Input::AudioMix(4)),
            InputInfo::buffer(Input::GainMix(4)),
            InputInfo::buffer(Input::LevelMix(4)),
            InputInfo::buffer(Input::AudioMix(5)),
            InputInfo::buffer(Input::GainMix(5)),
            InputInfo::buffer(Input::LevelMix(5)),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Buffer
    }

    fn process(&mut self, process_params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in process_params.active_voices {
                let voice = &mut channel.voices[*voice_idx];
                let router = VoiceRouter {
                    router,
                    module_id: self.id,
                    samples: process_params.samples,
                    voice_idx: *voice_idx,
                    channel_idx,
                };

                Self::process_voice(
                    &self.params,
                    &channel.params,
                    &mut self.buffers,
                    voice,
                    &router,
                );
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.channels[channel_idx].voices[voice_idx].output
    }
}
