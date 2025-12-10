use std::any::Any;

use crate::{
    synth_engine::{
        StereoSample,
        buffer::{Buffer, ONES_BUFFER, zero_buffer},
        routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
        synth_module::{
            InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams, SynthModule, VoiceAlive,
            VoiceRouter,
        },
        types::Sample,
    },
    utils::from_ms,
};
use itertools::izip;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    voice_kill_time: Sample,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            voice_kill_time: from_ms(30.0),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    level: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self { level: 1.0 }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AmplifierConfig {
    label: Option<String>,
    params: Params,
    channels: [ChannelParams; NUM_CHANNELS],
}

pub struct AmplifierUIData {
    pub label: String,
    pub level: StereoSample,
    pub voice_kill_time: Sample,
}

struct Voice {
    killed: bool,
    killed_output_power: Sample,
    killed_level: Sample,
    output: Buffer,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            killed: false,
            killed_level: 0.0,
            killed_output_power: 0.0,
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
    level_mod_input: Buffer,
}

pub struct Amplifier {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<AmplifierConfig>,
    params: Params,
    buffers: Buffers,
    channels: [Channel; NUM_CHANNELS],
}

impl Amplifier {
    pub fn new(id: ModuleId, config: ModuleConfigBox<AmplifierConfig>) -> Self {
        let mut amp = Self {
            id,
            label: format!("Amplifier {id}"),
            params: Params::default(),
            config,
            buffers: Buffers {
                input: zero_buffer(),
                level_mod_input: zero_buffer(),
            },
            channels: Default::default(),
        };

        load_module_config!(amp);
        amp
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> AmplifierUIData {
        AmplifierUIData {
            label: self.label.clone(),
            level: get_stereo_param!(self, level),
            voice_kill_time: self.params.voice_kill_time,
        }
    }

    set_mono_param!(set_voice_kill_time, voice_kill_time, Sample);
    set_stereo_param!(set_level, level);

    fn process_channel_voice(
        params: &Params,
        channel: &ChannelParams,
        sample_rate: Sample,
        voice: &mut Voice,
        buffers: &mut Buffers,
        router: &VoiceRouter,
    ) {
        let input = router.buffer(Input::Audio, &mut buffers.input);
        let level_mod = router
            .buffer_opt(Input::Level, &mut buffers.level_mod_input)
            .unwrap_or(&ONES_BUFFER);

        for (out, input, modulation) in izip!(
            voice.output.iter_mut().take(router.samples),
            input,
            level_mod
        ) {
            *out = input * channel.level * modulation;
        }

        if voice.killed {
            let kill_time = params.voice_kill_time.max(from_ms(4.0));
            let base = (-5.0 / (sample_rate * kill_time)).exp();
            let mut sum = 0.0;

            for out in voice.output.iter_mut().take(router.samples) {
                voice.killed_level *= base;
                *out *= voice.killed_level;
                sum += *out * *out;
            }

            voice.killed_output_power =
                (voice.killed_output_power + sum) / (router.samples + 1) as Sample;
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

    fn inputs(&self) -> &'static [InputInfo] {
        static INPUTS: &[InputInfo] = &[
            InputInfo::buffer(Input::Audio),
            InputInfo::buffer(Input::Level),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Buffer
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            let voice = &mut channel.voices[params.voice_idx];

            voice.killed = false;
            voice.killed_level = 1.0;
            voice.killed_output_power = 1.0;
        }
    }

    fn kill_voice(&mut self, voice_idx: usize) {
        for channel in &mut self.channels {
            channel.voices[voice_idx].killed = true;
        }
    }

    fn poll_alive_voices(&self, alive_state: &mut [VoiceAlive]) {
        const ALIVE_THRESHOLD: Sample = 0.0000001;

        for voice_alive in alive_state.iter_mut().filter(|alive| alive.killed()) {
            for channel in &self.channels {
                voice_alive.mark_alive(
                    channel.voices[voice_alive.index()].killed_output_power > ALIVE_THRESHOLD,
                );
            }
        }
    }

    fn process(&mut self, process_params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in process_params.active_voices {
                let router = VoiceRouter {
                    router,
                    module_id: self.id,
                    samples: process_params.samples,
                    voice_idx: *voice_idx,
                    channel_idx,
                };
                let voice = &mut channel.voices[*voice_idx];

                Self::process_channel_voice(
                    &self.params,
                    &channel.params,
                    process_params.sample_rate,
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
