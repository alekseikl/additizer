use core::f32;
use std::any::Any;
use std::array;

use nih_plug::util::db_to_gain_fast;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::SpectralBuffer,
    routing::{DataType, MAX_VOICES, MixType, NUM_CHANNELS, Router},
    synth_module::{InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams, VoiceRouter},
    types::{ComplexSample, SpectralOutput},
};

const MAX_INPUTS: usize = 6;
const MAX_VOLUME: Sample = 48.0; // dB

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    input_volumes: [Sample; MAX_INPUTS],
    output_volume: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            input_volumes: [0.0; MAX_INPUTS],
            output_volume: 0.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    num_inputs: usize,
    mix_types: [MixType; MAX_INPUTS],
}

impl Default for Params {
    fn default() -> Self {
        Self {
            num_inputs: 2,
            mix_types: Default::default(),
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
    pub mix_types: [MixType; MAX_INPUTS],
    pub input_volumes: [StereoSample; MAX_INPUTS],
    pub output_volume: StereoSample,
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
            mix_types: self.params.mix_types,
            input_volumes: array::from_fn(|idx| {
                self.channels
                    .iter()
                    .map(|channel| channel.params.input_volumes[idx])
                    .collect()
            }),
            output_volume: get_stereo_param!(self, output_volume),
        }
    }

    set_mono_param!(
        set_num_inputs,
        num_inputs,
        usize,
        num_inputs.clamp(1, MAX_INPUTS)
    );

    set_stereo_param!(set_output_volume, output_volume);

    pub fn set_mix_type(&mut self, input_idx: usize, mix_type: MixType) {
        self.params.mix_types[input_idx] = mix_type;
        self.config.lock().params.mix_types[input_idx] = mix_type;
    }

    pub fn set_input_volume(&mut self, input_idx: usize, volume: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS);

        for (channel, level) in self.channels.iter_mut().zip(volume.iter()) {
            channel.params.input_volumes[input_idx] = *level;
        }

        let mut cfg = self.config.lock();

        for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            config_channel.input_volumes[input_idx] = channel.params.input_volumes[input_idx];
        }
    }

    fn process_voice(
        current: bool,
        params: &Params,
        channel: &ChannelParams,
        voice: &mut Voice,
        router: &VoiceRouter,
    ) {
        fn to_gain(vol: Sample) -> Sample {
            db_to_gain_fast(vol.min(MAX_VOLUME))
        }

        let output = voice.output.advance();

        output.fill(ComplexSample::ZERO);

        for input_idx in 0..params.num_inputs {
            let spectrum = router.spectral(Input::SpectrumMix(input_idx), current);
            let gain = to_gain(
                channel.input_volumes[input_idx]
                    + router.scalar(Input::LevelMix(input_idx), current),
            );
            let iter = output.iter_mut().zip(spectrum.map(|input| input * gain));

            if input_idx == 0 {
                iter.for_each(|(out, input)| *out = input);
            } else {
                match params.mix_types[input_idx] {
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

        let output_gain = to_gain(channel.output_volume + router.scalar(Input::Level, current));

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
            InputInfo::scalar(Input::Level),
            InputInfo::spectral(Input::SpectrumMix(0)),
            InputInfo::scalar(Input::LevelMix(0)),
            InputInfo::spectral(Input::SpectrumMix(1)),
            InputInfo::scalar(Input::LevelMix(1)),
            InputInfo::spectral(Input::SpectrumMix(2)),
            InputInfo::scalar(Input::LevelMix(2)),
            InputInfo::spectral(Input::SpectrumMix(3)),
            InputInfo::scalar(Input::LevelMix(3)),
            InputInfo::spectral(Input::SpectrumMix(4)),
            InputInfo::scalar(Input::LevelMix(4)),
            InputInfo::spectral(Input::SpectrumMix(5)),
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
