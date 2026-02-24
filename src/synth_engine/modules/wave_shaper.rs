use itertools::izip;
use nih_plug::util::db_to_gain_fast;
use serde::{Deserialize, Serialize};
use std::any::Any;

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::{Buffer, zero_buffer},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
    synth_module::{ModInput, ModuleConfigBox, ProcessParams, VoiceRouter},
};

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ShaperType {
    #[default]
    HardClip,
    Sigmoid,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    distortion: Sample,
    clipping_level: Sample, // dB
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            distortion: 0.0,
            clipping_level: 0.0,
        }
    }
}
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Params {
    shaper_type: ShaperType,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct WaveShaperConfig {
    label: Option<String>,
    params: Params,
    channels: [ChannelParams; NUM_CHANNELS],
}

pub struct WaveShaperUIData {
    pub label: String,
    pub shaper_type: ShaperType,
    pub distortion: StereoSample,
    pub clipping_level: StereoSample,
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
    distortion_mod_input: Buffer,
    clipping_level_mod_input: Buffer,
}

pub struct WaveShaper {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<WaveShaperConfig>,
    buffers: Buffers,
    params: Params,
    channels: [Channel; NUM_CHANNELS],
}

impl WaveShaper {
    pub fn new(id: ModuleId, config: ModuleConfigBox<WaveShaperConfig>) -> Self {
        let mut ws = Self {
            id,
            label: format!("Waveshaper {id}"),
            config,
            buffers: Buffers {
                input: zero_buffer(),
                distortion_mod_input: zero_buffer(),
                clipping_level_mod_input: zero_buffer(),
            },
            params: Params::default(),
            channels: Default::default(),
        };

        load_module_config!(ws);
        ws
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> WaveShaperUIData {
        WaveShaperUIData {
            label: self.label.clone(),
            shaper_type: self.params.shaper_type,
            distortion: get_stereo_param!(self, distortion),
            clipping_level: get_stereo_param!(self, clipping_level),
        }
    }

    set_mono_param!(set_shaper_type, shaper_type, ShaperType);

    set_stereo_param!(set_distortion, distortion);
    set_stereo_param!(set_clipping_level, clipping_level);

    fn process_channel_voice(
        params: &Params,
        channel: &ChannelParams,
        voice: &mut Voice,
        buffers: &mut Buffers,
        router: &VoiceRouter,
    ) {
        let input = router.buffer(Input::Audio, &mut buffers.input);
        let clipping_level_mod =
            router.buffer(Input::ClippingLevel, &mut buffers.clipping_level_mod_input);
        let distortion_mod = router.buffer(Input::Distortion, &mut buffers.distortion_mod_input);

        for (out, input, clipping_level_mod, distortion_mod) in izip!(
            voice.output.iter_mut().take(router.samples),
            input,
            clipping_level_mod,
            distortion_mod
        ) {
            let clipping_gain =
                db_to_gain_fast((channel.clipping_level + clipping_level_mod).min(24.0));
            let gain = db_to_gain_fast((channel.distortion + distortion_mod).clamp(0.0, 48.0));

            match params.shaper_type {
                ShaperType::HardClip => {
                    *out = (input * gain).clamp(-clipping_gain, clipping_gain);
                }
                ShaperType::Sigmoid => {
                    *out = clipping_gain
                        * (2.0 / (1.0 + (-2.0 * input * gain / clipping_gain).exp()) - 1.0)
                }
            }
        }
    }
}

impl SynthModule for WaveShaper {
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
        ModuleType::WaveShaper
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::buffer(Input::Audio),
            ModInput::buffer(Input::ClippingLevel),
            ModInput::buffer(Input::Distortion),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Buffer
    }

    fn process(&mut self, process_params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in process_params.active_voices {
                let router = VoiceRouter {
                    router,
                    module_id: self.id,
                    samples: process_params.samples,
                    sample_rate: process_params.sample_rate,
                    voice_idx: *voice_idx,
                    channel_idx,
                };
                let voice = &mut channel.voices[*voice_idx];

                Self::process_channel_voice(
                    &self.params,
                    &channel.params,
                    voice,
                    &mut self.buffers,
                    &router,
                );
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
    }
}
