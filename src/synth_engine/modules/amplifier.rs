use std::array;

use itertools::izip;

mod config;
mod link;
mod ui_bridge;

pub use config::AmplifierConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::AmplifierUiBridge;

use crate::synth_engine::{
    StereoSample,
    buffer::{Buffer, zero_buffer},
    routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
    smooth::SmoothedSample,
    synth_module::{ModInput, ProcessParams, SynthModule, VoiceRouter, VoiceRouterFactory},
};

struct ChannelParams {
    gain: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &AmplifierConfig, channel_idx: usize) -> Self {
        Self {
            gain: c.gain[channel_idx].into(),
        }
    }
}

struct Voice {
    output: Buffer,
}

impl Voice {
    fn new() -> Self {
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

struct Buffers {
    input: Buffer,
    gain_mod_input: Buffer,
}

type ChannelVoices = [Voice; MAX_VOICES];

pub struct Amplifier {
    id: ModuleId,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: [ChannelVoices; NUM_CHANNELS],
}

impl Amplifier {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&AmplifierConfig {
            id,
            ..AmplifierConfig::default()
        })
    }

    pub fn from_config(config: &config::AmplifierConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers {
                input: zero_buffer(),
                gain_mod_input: zero_buffer(),
            },
            audio_end,
            ui_end: Some(ui_end),
            voices: Default::default(),
        }
    }

    pub fn get_config(&self) -> AmplifierConfig {
        AmplifierConfig {
            id: self.id,
            gain: get_smoothed_param!(self, gain),
        }
    }

    set_smoothed_param!(set_gain, gain);

    fn process_channel_voice(&mut self, mut router: VoiceRouter<'_, '_>) {
        let channel = &mut self.channel_params[router.channel_idx()];
        let voice = &mut self.voices[router.channel_idx()][router.voice_idx()];

        router.buff_param(
            Input::Gain,
            &mut channel.gain,
            &mut self.buffers.gain_mod_input,
        );

        let input = router.buffer(Input::Audio, &mut self.buffers.input);

        for (out, input, modulation) in
            izip!(voice.output.iter_mut(), input, self.buffers.gain_mod_input)
                .take(router.samples())
        {
            *out = input * modulation;
        }
    }
}

impl SynthModule for Amplifier {
    fn id(&self) -> ModuleId {
        self.id
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

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => {
                    if input == Input::Gain {
                        self.set_gain(value)
                    }
                }
            }
        }
    }

    fn process(&mut self, process_params: &ProcessParams, router: &mut dyn Router) {
        let mut rf = VoiceRouterFactory::new(self.id, router, process_params);

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                self.process_channel_voice(rf.for_voice(*voice_idx, channel_idx, seq_idx));
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.voices[channel][voice_idx].output
    }
}
