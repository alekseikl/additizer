use std::array;

use nih_plug::util::db_to_gain_fast;

mod config;
mod link;
mod ui_bridge;

pub use config::{MixerConfig, MAX_INPUTS};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::MixerUiBridge;

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule, VolumeType,
    buffer::{Buffer, copy_or_add_to_buffer, new_channels_layout, zero_buffer},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
    synth_module::{ModInput, ProcessParams, VoiceRouter, VoiceRouterFactory},
};

const MAX_VOLUME: Sample = 24.0; // dB

struct InputChannelParams {
    level: Sample,
    gain: Sample,
}

struct ChannelParams {
    input_params: [InputChannelParams; MAX_INPUTS as usize],
    output_level: Sample,
    output_gain: Sample,
}

impl ChannelParams {
    fn from_config(c: &config::MixerConfig, channel_idx: usize) -> Self {
        Self {
            input_params: c.inputs.map(|input| InputChannelParams {
                level: input.level[channel_idx],
                gain: input.gain[channel_idx],
            }),
            output_level: c.output_level[channel_idx],
            output_gain: c.output_gain[channel_idx],
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

type ChannelVoices = [Voice; MAX_VOICES];

pub struct Mixer {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: Box<[ChannelVoices; NUM_CHANNELS]>,
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
            voices: new_channels_layout(),
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
                        .map(|channel| channel.input_params[input_idx].level),
                ),
                gain: StereoSample::from_iter(
                    self.channel_params
                        .iter()
                        .map(|channel| channel.input_params[input_idx].gain),
                ),
            }),
            output_volume_type: self.params.output_volume_type,
            output_level: get_stereo_param!(self, output_level),
            output_gain: get_stereo_param!(self, output_gain),
        }
    }

    set_mono_param!(
        set_num_inputs,
        num_inputs,
        u8,
        num_inputs.clamp(1, MAX_INPUTS)
    );

    set_mono_param!(set_output_volume_type, output_volume_type, VolumeType);

    set_stereo_param!(set_output_level, output_level);
    set_stereo_param!(set_output_gain, output_gain);

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;
        self.params.inputs[input_idx].volume_type = volume_type;
    }

    pub fn set_input_level(&mut self, input_idx: u8, level: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, level) in self.channel_params.iter_mut().zip(level.iter()) {
            channel.input_params[input_idx].level = *level;
        }
    }

    pub fn set_input_gain(&mut self, input_idx: u8, gain: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, gain) in self.channel_params.iter_mut().zip(gain.iter()) {
            channel.input_params[input_idx].gain = *gain;
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
        input_idx: u8,
        samples: usize,
    ) {
        let input = input
            .iter()
            .zip(gain_mod)
            .map(|(sample, gain_mod)| sample * gain_mod);

        copy_or_add_to_buffer(input_idx == 0, output, input.take(samples));
    }

    #[inline(always)]
    fn modulate_output(
        output: &mut Buffer,
        gain_mod: impl Iterator<Item = Sample>,
        samples: usize,
    ) {
        for (out, gain_mod) in output.iter_mut().zip(gain_mod).take(samples) {
            *out *= gain_mod;
        }
    }

    fn process_voice(&mut self, router: &mut VoiceRouter<'_, '_>) {
        let channel = &self.channel_params[router.channel_idx()];
        let voice = &mut self.voices[router.channel_idx()][router.voice_idx()];
        let samples = router.samples();

        for input_idx in 0..self.params.num_inputs {
            let input_params = &self.params.inputs[input_idx as usize];
            let input_channel = &channel.input_params[input_idx as usize];
            let input = router.buffer(Input::AudioMix(input_idx), &mut self.buffers.input);

            match input_params.volume_type {
                VolumeType::Db => {
                    let level_mod =
                        router.buffer(Input::LevelMix(input_idx), &mut self.buffers.level_mod);
                    let gain_mod = level_mod
                        .iter()
                        .map(|level_mod| Self::to_gain(input_channel.level + level_mod));

                    Self::mix_input(&mut voice.output, input, gain_mod, input_idx, samples);
                }
                VolumeType::Gain => {
                    let gain_mod =
                        router.buffer(Input::GainMix(input_idx), &mut self.buffers.level_mod);
                    let gain_mod = gain_mod
                        .iter()
                        .map(|gain_mod| input_channel.gain + gain_mod);

                    Self::mix_input(&mut voice.output, input, gain_mod, input_idx, samples);
                }
            }
        }

        match self.params.output_volume_type {
            VolumeType::Db => {
                let level_mod = router.buffer(Input::Level, &mut self.buffers.level_mod);
                let gain_mod = level_mod
                    .iter()
                    .map(|level_mod| Self::to_gain(channel.output_level + level_mod));

                Self::modulate_output(&mut voice.output, gain_mod, samples);
            }
            VolumeType::Gain => {
                let gain_mod = router.buffer(Input::Gain, &mut self.buffers.level_mod);
                let gain_mod = gain_mod
                    .iter()
                    .map(|gain_mod| channel.output_gain + gain_mod);

                Self::modulate_output(&mut voice.output, gain_mod, samples);
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
            ModInput::buffer(Input::Gain),
            ModInput::buffer(Input::Level),
            ModInput::buffer(Input::AudioMix(0)),
            ModInput::buffer(Input::GainMix(0)),
            ModInput::buffer(Input::LevelMix(0)),
            ModInput::buffer(Input::AudioMix(1)),
            ModInput::buffer(Input::GainMix(1)),
            ModInput::buffer(Input::LevelMix(1)),
            ModInput::buffer(Input::AudioMix(2)),
            ModInput::buffer(Input::GainMix(2)),
            ModInput::buffer(Input::LevelMix(2)),
            ModInput::buffer(Input::AudioMix(3)),
            ModInput::buffer(Input::GainMix(3)),
            ModInput::buffer(Input::LevelMix(3)),
            ModInput::buffer(Input::AudioMix(4)),
            ModInput::buffer(Input::GainMix(4)),
            ModInput::buffer(Input::LevelMix(4)),
            ModInput::buffer(Input::AudioMix(5)),
            ModInput::buffer(Input::GainMix(5)),
            ModInput::buffer(Input::LevelMix(5)),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Buffer
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

    fn process(&mut self, process_params: &ProcessParams, router: &mut dyn Router) {
        let mut rf = VoiceRouterFactory::new(self.id, router, process_params);

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                let mut voice_router = rf.for_voice(*voice_idx, channel_idx, seq_idx);

                self.process_voice(&mut voice_router);
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.voices[channel_idx][voice_idx].output
    }
}
