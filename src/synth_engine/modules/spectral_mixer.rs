use core::f32;
use std::array;

use nih_plug::util::db_to_gain_fast;

mod config;
mod link;
mod ui_bridge;

pub use config::{MAX_INPUTS, SpectralMixerConfig};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::UiBridge;

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::SpectralBuffer,
    routing::{DataType, MAX_VOICES, MixType, NUM_CHANNELS, Router, VoiceEvent, VolumeType},
    synth_module::{ModInput, ProcessParams, VoiceRouter, VoiceRouterFactory},
    types::{ComplexSample, SpectralOutput},
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
    fn from_config(c: &config::SpectralMixerConfig, channel_idx: usize) -> Self {
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
    mix_type: MixType,
    volume_type: VolumeType,
}

struct Params {
    num_inputs: u8,
    inputs: [InputParams; MAX_INPUTS as usize],
    output_volume_type: VolumeType,
}

impl Params {
    fn from_config(c: &config::SpectralMixerConfig) -> Self {
        Self {
            num_inputs: c.num_inputs,
            inputs: c.inputs.map(|input| InputParams {
                mix_type: input.mix_type,
                volume_type: input.volume_type,
            }),
            output_volume_type: c.output_volume_type,
        }
    }
}

#[derive(Default)]
struct Voice {
    triggered: bool,
    output: SpectralOutput,
}

type ChannelVoices = [Voice; MAX_VOICES];

pub struct SpectralMixer {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: [ChannelVoices; NUM_CHANNELS],
}

impl SpectralMixer {
    pub const MAX_INPUTS: u8 = MAX_INPUTS;

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SpectralMixerConfig {
            id,
            ..SpectralMixerConfig::default()
        })
    }

    pub fn from_config(config: &config::SpectralMixerConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            audio_end,
            ui_end: Some(ui_end),
            voices: Default::default(),
        }
    }

    pub fn take_ui_end(&mut self) -> Option<UiEnd> {
        self.ui_end.take()
    }

    pub fn return_ui_end(&mut self, ui_end: UiEnd) {
        assert!(self.ui_end.is_none(), "ui_end not taken");
        self.ui_end = Some(ui_end);
    }

    pub fn get_config(&self) -> SpectralMixerConfig {
        SpectralMixerConfig {
            id: self.id,
            num_inputs: self.params.num_inputs,
            inputs: array::from_fn(|input_idx| config::InputConfig {
                mix_type: self.params.inputs[input_idx].mix_type,
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

    pub fn set_mix_type(&mut self, input_idx: u8, mix_type: MixType) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;
        self.params.inputs[input_idx].mix_type = mix_type;
    }

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
    fn to_gain(vol: Sample) -> Sample {
        db_to_gain_fast(vol.min(MAX_VOLUME))
    }

    fn process_voice(&mut self, router: &mut VoiceRouter<'_, '_>) {
        let channel = &self.channel_params[router.channel_idx()];
        let voice = &mut self.voices[router.channel_idx()][router.voice_idx()];
        let current = !voice.triggered;
        let output = voice.output.advance();

        output.fill(ComplexSample::ZERO);

        for input_idx in 0..self.params.num_inputs {
            let input_params = &self.params.inputs[input_idx as usize];
            let input_channel = &channel.input_params[input_idx as usize];

            let gain = match input_params.volume_type {
                VolumeType::Db => Self::to_gain(router.scalar(
                    Input::LevelMix(input_idx),
                    input_channel.level,
                    current,
                )),
                VolumeType::Gain => {
                    router.scalar(Input::GainMix(input_idx), input_channel.gain, current)
                }
            };

            let spectrum = router.spectral(Input::SpectrumMix(input_idx), current);

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

        let output_gain = match self.params.output_volume_type {
            VolumeType::Db => {
                Self::to_gain(router.scalar(Input::Level, channel.output_level, current))
            }
            VolumeType::Gain => router.scalar(Input::Gain, channel.output_gain, current),
        };

        for out in output.iter_mut() {
            *out *= output_gain;
        }

        if voice.triggered {
            voice.triggered = false;

            self.process_voice(router);
        }
    }
}

impl SynthModule for SpectralMixer {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::SpectralMixer
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::scalar(Input::Gain),
            ModInput::scalar(Input::Level),
            ModInput::spectral(Input::SpectrumMix(0)),
            ModInput::scalar(Input::GainMix(0)),
            ModInput::scalar(Input::LevelMix(0)),
            ModInput::spectral(Input::SpectrumMix(1)),
            ModInput::scalar(Input::GainMix(1)),
            ModInput::scalar(Input::LevelMix(1)),
            ModInput::spectral(Input::SpectrumMix(2)),
            ModInput::scalar(Input::GainMix(2)),
            ModInput::scalar(Input::LevelMix(2)),
            ModInput::spectral(Input::SpectrumMix(3)),
            ModInput::scalar(Input::GainMix(3)),
            ModInput::scalar(Input::LevelMix(3)),
            ModInput::spectral(Input::SpectrumMix(4)),
            ModInput::scalar(Input::GainMix(4)),
            ModInput::scalar(Input::LevelMix(4)),
            ModInput::spectral(Input::SpectrumMix(5)),
            ModInput::scalar(Input::GainMix(5)),
            ModInput::scalar(Input::LevelMix(5)),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Spectral
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in &mut self.voices {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    channel[*voice_idx].triggered = true;
                }
            }
        }
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
                UiEvent::MixType {
                    input_idx,
                    mix_type,
                } => self.set_mix_type(input_idx, mix_type),
                UiEvent::VolumeType {
                    input_idx,
                    volume_type,
                } => self.set_volume_type(input_idx, volume_type),
                UiEvent::OutputVolumeType(volume_type) => self.set_output_volume_type(volume_type),
            }
        }
    }

    fn process(&mut self, process_params: &ProcessParams, router: &mut dyn Router) {
        let mut rf = VoiceRouterFactory::new(self.id, router, process_params);

        for channel_idx in (0..NUM_CHANNELS).take(process_params.spectrum_channels) {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                let mut voice_router = rf.for_voice(*voice_idx, channel_idx, seq_idx);

                self.process_voice(&mut voice_router);
            }
        }
    }

    fn get_spectral_output(
        &self,
        current: bool,
        voice_idx: usize,
        channel_idx: usize,
    ) -> &SpectralBuffer {
        self.voices[channel_idx][voice_idx].output.get(current)
    }
}
