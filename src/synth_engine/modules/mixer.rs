use std::array;

use nih_plug::util::db_to_gain_fast;
use serde::{Deserialize, Serialize};

mod link;
mod ui_bridge;

use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::{ControlsState, UiBridge};

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule, VolumeType,
    buffer::{Buffer, copy_or_add_to_buffer, zero_buffer},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
    synth_module::{ModInput, ModuleConfigBox, ProcessParams, VoiceRouter, VoiceRouterFactory},
};

pub const MAX_INPUTS: u8 = 6;
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
    input_params: [InputChannelParams; MAX_INPUTS as usize],
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
    num_inputs: u8,
    input_volume_types: [VolumeType; MAX_INPUTS as usize],
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
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    channels: [Channel; NUM_CHANNELS],
}

impl Mixer {
    pub const MAX_INPUTS: u8 = MAX_INPUTS;

    pub fn new(id: ModuleId, config: ModuleConfigBox<MixerConfig>) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        let mut mixer = Self {
            id,
            label: format!("Mixer {id}"),
            config,
            params: Params::default(),
            buffers: Buffers::default(),
            audio_end,
            ui_end: Some(ui_end),
            channels: Default::default(),
        };

        load_module_config!(mixer);
        mixer
    }

    pub fn take_ui_end(&mut self) -> Option<UiEnd> {
        self.ui_end.take()
    }

    pub fn return_ui_end(&mut self, ui_end: UiEnd) {
        assert!(self.ui_end.is_none(), "ui_end not taken");
        self.ui_end = Some(ui_end);
    }

    pub fn get_controls_state(&self) -> ControlsState {
        ControlsState {
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
        u8,
        num_inputs.clamp(1, MAX_INPUTS)
    );

    set_mono_param!(set_output_volume_type, output_volume_type, VolumeType);

    set_stereo_param!(set_output_level, output_level);
    set_stereo_param!(set_output_gain, output_gain);

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;
        self.params.input_volume_types[input_idx] = volume_type;
        self.config.lock().params.input_volume_types[input_idx] = volume_type;
    }

    pub fn set_input_level(&mut self, input_idx: u8, level: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, level) in self.channels.iter_mut().zip(level.iter()) {
            channel.params.input_params[input_idx].level = *level;
        }

        let mut cfg = self.config.lock();

        for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            config_channel.input_params[input_idx].level =
                channel.params.input_params[input_idx].level;
        }
    }

    pub fn set_input_gain(&mut self, input_idx: u8, gain: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

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
        input_idx: u8,
        samples: usize,
    ) {
        let input = input
            .iter()
            .zip(gain_mod)
            .map(|(sample, gain_mod)| sample * gain_mod);

        copy_or_add_to_buffer(input_idx == 0, output, input.take(samples));
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
        router: &VoiceRouter<'_, '_>,
    ) {
        let samples = router.samples();

        for input_idx in 0..params.num_inputs {
            let input = router.buffer(Input::AudioMix(input_idx), &mut buffers.input);

            match params.input_volume_types[input_idx as usize] {
                VolumeType::Db => {
                    let channel_level = channel.input_params[input_idx as usize].level;
                    let level_mod =
                        router.buffer(Input::LevelMix(input_idx), &mut buffers.level_mod);
                    let gain_mod = level_mod
                        .iter()
                        .map(|level_mod| Self::to_gain(channel_level + level_mod));

                    Self::mix_input(&mut voice.output, input, gain_mod, input_idx, samples);
                }
                VolumeType::Gain => {
                    let channel_gain = channel.input_params[input_idx as usize].gain;
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

        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                let voice = &mut channel.voices[*voice_idx];
                let voice_router = rf.for_voice(*voice_idx, channel_idx, seq_idx);

                Self::process_voice(
                    &self.params,
                    &channel.params,
                    &mut self.buffers,
                    voice,
                    &voice_router,
                );
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.channels[channel_idx].voices[voice_idx].output
    }
}
