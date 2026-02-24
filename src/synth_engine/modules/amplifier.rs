use std::any::Any;

use crate::synth_engine::{
    StereoSample,
    buffer::{Buffer, zero_buffer},
    routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
    synth_module::{ModInput, ModuleConfigBox, ProcessParams, SynthModule, VoiceRouter},
    types::Sample,
};
use itertools::izip;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    gain: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self { gain: 0.0 }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AmplifierConfig {
    label: Option<String>,
    channels: [ChannelParams; NUM_CHANNELS],
}

pub struct AmplifierUIData {
    pub label: String,
    pub gain: StereoSample,
}

struct Voice {
    output: Buffer,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            output: zero_buffer(),
        }
    }
}

impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

struct Buffers {
    input: Buffer,
    gain_mod_input: Buffer,
}

pub struct Amplifier {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<AmplifierConfig>,
    buffers: Buffers,
    channels: [Channel; NUM_CHANNELS],
}

impl Amplifier {
    pub fn new(id: ModuleId, config: ModuleConfigBox<AmplifierConfig>) -> Self {
        let mut amp = Self {
            id,
            label: format!("Amplifier {id}"),
            config,
            buffers: Buffers {
                input: zero_buffer(),
                gain_mod_input: zero_buffer(),
            },
            channels: Default::default(),
        };

        load_module_config_no_params!(amp);
        amp
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> AmplifierUIData {
        AmplifierUIData {
            label: self.label.clone(),
            gain: get_stereo_param!(self, gain),
        }
    }

    set_stereo_param!(set_gain, gain);

    fn process_channel_voice(
        channel: &ChannelParams,
        voice: &mut Voice,
        buffers: &mut Buffers,
        router: &VoiceRouter,
    ) {
        let input = router.buffer(Input::Audio, &mut buffers.input);
        let gain_mod = router.buffer(Input::Gain, &mut buffers.gain_mod_input);

        for (out, input, modulation) in izip!(
            voice.output.iter_mut().take(router.samples),
            input,
            gain_mod
        ) {
            *out = input * (channel.gain + modulation);
        }
    }
}

impl SynthModule for Amplifier {
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
        ModuleType::Amplifier
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::buffer(Input::Audio),
            ModInput::buffer(Input::Gain),
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

                Self::process_channel_voice(&channel.params, voice, &mut self.buffers, &router);
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
    }
}
