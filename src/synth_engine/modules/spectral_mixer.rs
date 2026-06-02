use core::f32;
use std::array;

use nih_plug::util::db_to_gain_fast;
use serde::{Deserialize, Serialize};

mod link;
mod ui_bridge;

use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::{ControlsState, UiBridge};

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::SpectralBuffer,
    routing::{DataType, MAX_VOICES, MixType, NUM_CHANNELS, Router, VoiceEvent, VolumeType},
    synth_module::{ModInput, ModuleConfigBox, ProcessParams, VoiceRouter, VoiceRouterFactory},
    types::{ComplexSample, SpectralOutput},
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

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct InputParams {
    pub mix_type: MixType,
    pub volume_type: VolumeType,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    num_inputs: u8,
    input_params: [InputParams; MAX_INPUTS as usize],
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
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    channels: [Channel; NUM_CHANNELS],
}

impl SpectralMixer {
    pub const MAX_INPUTS: u8 = MAX_INPUTS;

    pub fn new(id: ModuleId, config: ModuleConfigBox<SpectralMixerConfig>) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        let mut mixer = Self {
            id,
            label: format!("Spectral Mixer {id}"),
            config,
            params: Params::default(),
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
        u8,
        num_inputs.clamp(1, MAX_INPUTS)
    );

    set_mono_param!(set_output_volume_type, output_volume_type, VolumeType);

    set_stereo_param!(set_output_level, output_level);
    set_stereo_param!(set_output_gain, output_gain);

    pub fn set_mix_type(&mut self, input_idx: u8, mix_type: MixType) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;
        self.params.input_params[input_idx].mix_type = mix_type;
        self.config.lock().params.input_params[input_idx].mix_type = mix_type;
    }

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;
        self.params.input_params[input_idx].volume_type = volume_type;
        self.config.lock().params.input_params[input_idx].volume_type = volume_type;
    }

    pub fn set_input_level(&mut self, input_idx: u8, volume: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, level) in self.channels.iter_mut().zip(volume.iter()) {
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
    fn to_gain(vol: Sample) -> Sample {
        db_to_gain_fast(vol.min(MAX_VOLUME))
    }

    fn process_voice(
        current: bool,
        params: &Params,
        channel: &ChannelParams,
        voice: &mut Voice,
        router: &mut VoiceRouter<'_, '_>,
    ) {
        let output = voice.output.advance();

        output.fill(ComplexSample::ZERO);

        for input_idx in 0..params.num_inputs {
            let input_params = &params.input_params[input_idx as usize];
            let channel_input_params = &channel.input_params[input_idx as usize];

            let gain = match input_params.volume_type {
                VolumeType::Db => Self::to_gain(router.scalar(
                    Input::LevelMix(input_idx),
                    channel_input_params.level,
                    current,
                )),
                VolumeType::Gain => router.scalar(
                    Input::GainMix(input_idx),
                    channel_input_params.gain,
                    current,
                ),
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

        let output_gain = match params.output_volume_type {
            VolumeType::Db => {
                Self::to_gain(router.scalar(Input::Level, channel.output_level, current))
            }
            VolumeType::Gain => router.scalar(Input::Gain, channel.output_gain, current),
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
        for channel in &mut self.channels {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    channel.voices[*voice_idx].triggered = true;
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

        for (channel_idx, channel) in self
            .channels
            .iter_mut()
            .enumerate()
            .take(process_params.spectrum_channels)
        {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                let voice = &mut channel.voices[*voice_idx];
                let mut voice_router = rf.for_voice(*voice_idx, channel_idx, seq_idx);

                if voice.triggered {
                    Self::process_voice(
                        false,
                        &self.params,
                        &channel.params,
                        voice,
                        &mut voice_router,
                    );
                    voice.triggered = false;
                }
                Self::process_voice(
                    true,
                    &self.params,
                    &channel.params,
                    voice,
                    &mut voice_router,
                );
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
