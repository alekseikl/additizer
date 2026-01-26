use core::f32;
use std::any::Any;
use std::array;

use nih_plug::util::db_to_gain_fast;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::SpectralBuffer,
    routing::{DataType, MAX_VOICES, MixType, NUM_CHANNELS, Router, VolumeType},
    synth_module::{InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams, VoiceRouter},
    types::{ComplexSample, SpectralOutput},
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

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct InputParams {
    pub mix_type: MixType,
    pub volume_type: VolumeType,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    num_inputs: usize,
    input_params: [InputParams; MAX_INPUTS],
    output_volume_type: VolumeType,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            num_inputs: 2,
            input_params: Default::default(),
            output_volume_type: VolumeType::Gain,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SpectralMixerConfig {
    label: Option<String>,
    params: Params,
    channels: [ChannelParams; NUM_CHANNELS],
}

pub struct SpectralMixerUIData {
    pub label: String,
    pub num_inputs: usize,
    pub input_params: [InputParams; MAX_INPUTS],
    pub input_levels: [StereoSample; MAX_INPUTS],
    pub input_gains: [StereoSample; MAX_INPUTS],
    pub output_volume_type: VolumeType,
    pub output_level: StereoSample,
    pub output_gain: StereoSample,
}

#[derive(Default)]
struct Voice {
    triggered: bool,
    output: SpectralOutput,
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

pub struct SpectralMixer {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<SpectralMixerConfig>,
    params: Params,
    channels: [Channel; NUM_CHANNELS],
}

impl SpectralMixer {
    pub const MAX_INPUTS: usize = MAX_INPUTS;

    pub fn new(id: ModuleId, config: ModuleConfigBox<SpectralMixerConfig>) -> Self {
        let mut mixer = Self {
            id,
            label: format!("Spectral Mixer {id}"),
            config,
            params: Params::default(),
            channels: Default::default(),
        };

        load_module_config!(mixer);
        mixer
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> SpectralMixerUIData {
        SpectralMixerUIData {
            label: self.label.clone(),
            num_inputs: self.params.num_inputs,
            input_params: self.params.input_params,
            input_levels: array::from_fn(|idx| {
                self.channels
                    .iter()
                    .map(|channel| channel.params.input_params[idx].level)
                    .collect()
            }),
            input_gains: array::from_fn(|idx| {
                self.channels
                    .iter()
                    .map(|channel| channel.params.input_params[idx].gain)
                    .collect()
            }),
            output_volume_type: self.params.output_volume_type,
            output_level: get_stereo_param!(self, output_level),
            output_gain: get_stereo_param!(self, output_gain),
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

    pub fn set_mix_type(&mut self, input_idx: usize, mix_type: MixType) {
        self.params.input_params[input_idx].mix_type = mix_type;
        self.config.lock().params.input_params[input_idx].mix_type = mix_type;
    }

    pub fn set_volume_type(&mut self, input_idx: usize, volume_type: VolumeType) {
        self.params.input_params[input_idx].volume_type = volume_type;
        self.config.lock().params.input_params[input_idx].volume_type = volume_type;
    }

    pub fn set_input_level(&mut self, input_idx: usize, volume: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS);

        for (channel, level) in self.channels.iter_mut().zip(volume.iter()) {
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
    fn to_gain(vol: Sample) -> Sample {
        db_to_gain_fast(vol.min(MAX_VOLUME))
    }

    fn process_voice(
        current: bool,
        params: &Params,
        channel: &ChannelParams,
        voice: &mut Voice,
        router: &VoiceRouter,
    ) {
        let output = voice.output.advance();

        output.fill(ComplexSample::ZERO);

        for input_idx in 0..params.num_inputs {
            let input_params = &params.input_params[input_idx];
            let channel_input_params = &channel.input_params[input_idx];
            let spectrum = router.spectral(Input::SpectrumMix(input_idx), current);

            let gain = match input_params.volume_type {
                VolumeType::Db => Self::to_gain(
                    channel_input_params.level + router.scalar(Input::LevelMix(input_idx), current),
                ),
                VolumeType::Gain => {
                    channel_input_params.gain + router.scalar(Input::GainMix(input_idx), current)
                }
            };

            let iter = output.iter_mut().zip(spectrum.map(|input| input * gain));

            if input_idx == 0 {
                iter.for_each(|(out, input)| *out = input);
            } else {
                match input_params.mix_type {
                    MixType::Add => {
                        iter.for_each(|(out, input)| *out += input);
                    }
                    MixType::Subtract => {
                        iter.for_each(|(out, input)| *out -= input);
                    }
                    MixType::Multiply => {
                        iter.enumerate().for_each(|(idx, (out, input))| {
                            *out *= input * idx as Sample * f32::consts::PI
                        });
                    }
                }
            }
        }

        let output_gain = match params.output_volume_type {
            VolumeType::Db => {
                Self::to_gain(channel.output_level + router.scalar(Input::Level, current))
            }
            VolumeType::Gain => channel.output_gain + router.scalar(Input::Gain, current),
        };

        for out in output.iter_mut() {
            *out *= output_gain;
        }
    }
}

impl SynthModule for SpectralMixer {
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
        ModuleType::SpectralMixer
    }

    fn inputs(&self) -> &'static [InputInfo] {
        static INPUTS: &[InputInfo] = &[
            InputInfo::scalar(Input::Gain),
            InputInfo::scalar(Input::Level),
            InputInfo::spectral(Input::SpectrumMix(0)),
            InputInfo::scalar(Input::GainMix(0)),
            InputInfo::scalar(Input::LevelMix(0)),
            InputInfo::spectral(Input::SpectrumMix(1)),
            InputInfo::scalar(Input::GainMix(1)),
            InputInfo::scalar(Input::LevelMix(1)),
            InputInfo::spectral(Input::SpectrumMix(2)),
            InputInfo::scalar(Input::GainMix(2)),
            InputInfo::scalar(Input::LevelMix(2)),
            InputInfo::spectral(Input::SpectrumMix(3)),
            InputInfo::scalar(Input::GainMix(3)),
            InputInfo::scalar(Input::LevelMix(3)),
            InputInfo::spectral(Input::SpectrumMix(4)),
            InputInfo::scalar(Input::GainMix(4)),
            InputInfo::scalar(Input::LevelMix(4)),
            InputInfo::spectral(Input::SpectrumMix(5)),
            InputInfo::scalar(Input::GainMix(5)),
            InputInfo::scalar(Input::LevelMix(5)),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Spectral
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            channel.voices[params.voice_idx].triggered = true;
        }
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

                if voice.triggered {
                    Self::process_voice(false, &self.params, &channel.params, voice, &router);
                    voice.triggered = false;
                }
                Self::process_voice(true, &self.params, &channel.params, voice, &router);
            }
        }
    }

    fn get_spectral_output(
        &self,
        current: bool,
        voice_idx: usize,
        channel_idx: usize,
    ) -> &SpectralBuffer {
        self.channels[channel_idx].voices[voice_idx]
            .output
            .get(current)
    }
}
