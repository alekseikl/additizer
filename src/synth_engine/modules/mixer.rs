use std::any::Any;
use std::array;

use nih_plug::util::db_to_gain_fast;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::{Buffer, copy_or_add_buffer, zero_buffer},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
    synth_module::{InputInfo, ModuleConfigBox, ProcessParams, VoiceRouter},
};

const MAX_INPUTS: usize = 6;
const MAX_VOLUME: Sample = 24.0; // dB

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    input_levels: [Sample; MAX_INPUTS],
    output_level: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            input_levels: [0.0; MAX_INPUTS],
            output_level: 0.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    num_inputs: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self { num_inputs: 2 }
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
    pub input_levels: [StereoSample; MAX_INPUTS],
    pub output_level: StereoSample,
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
            input_levels: array::from_fn(|idx| {
                self.channels
                    .iter()
                    .map(|channel| channel.params.input_levels[idx])
                    .collect()
            }),
            output_level: get_stereo_param!(self, output_level),
        }
    }

    set_mono_param!(
        set_num_inputs,
        num_inputs,
        usize,
        num_inputs.clamp(1, MAX_INPUTS)
    );

    set_stereo_param!(set_output_level, output_level);

    pub fn set_input_level(&mut self, input_idx: usize, level: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS);

        for (channel, level) in self.channels.iter_mut().zip(level.iter()) {
            channel.params.input_levels[input_idx] = *level;
        }

        let mut cfg = self.config.lock();

        for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            config_channel.input_levels[input_idx] = channel.params.input_levels[input_idx];
        }
    }

    #[inline(always)]
    fn to_gain(dbs: Sample) -> Sample {
        db_to_gain_fast(dbs.min(MAX_VOLUME))
    }

    fn process_voice(
        params: &Params,
        channel: &ChannelParams,
        buffers: &mut Buffers,
        voice: &mut Voice,
        router: &VoiceRouter,
    ) {
        for input_idx in 0..params.num_inputs {
            let channel_level = channel.input_levels[input_idx];
            let audio = router.buffer(Input::AudioMix(input_idx), &mut buffers.input);
            let level_mod = router.buffer(Input::LevelMix(input_idx), &mut buffers.level_mod);
            let input = audio
                .iter()
                .zip(level_mod)
                .map(|(sample, level_mod)| sample * Self::to_gain(channel_level + level_mod));

            copy_or_add_buffer(
                input_idx == 0,
                &mut voice.output,
                input.take(router.samples),
            );
        }

        let output_level_mod = router.buffer(Input::Level, &mut buffers.level_mod);

        for (out, level_mod) in voice
            .output
            .iter_mut()
            .take(router.samples)
            .zip(output_level_mod)
        {
            *out *= Self::to_gain(channel.output_level + level_mod);
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
            InputInfo::buffer(Input::Level),
            InputInfo::buffer(Input::AudioMix(0)),
            InputInfo::buffer(Input::LevelMix(0)),
            InputInfo::buffer(Input::AudioMix(1)),
            InputInfo::buffer(Input::LevelMix(1)),
            InputInfo::buffer(Input::AudioMix(2)),
            InputInfo::buffer(Input::LevelMix(2)),
            InputInfo::buffer(Input::AudioMix(3)),
            InputInfo::buffer(Input::LevelMix(3)),
            InputInfo::buffer(Input::AudioMix(4)),
            InputInfo::buffer(Input::LevelMix(4)),
            InputInfo::buffer(Input::AudioMix(5)),
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
