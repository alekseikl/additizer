use std::any::Any;

use crate::{
    synth_engine::{
        StereoSample,
        buffer::{Buffer, ONES_BUFFER, ZEROES_BUFFER, zero_buffer},
        routing::{
            DataType, Input, MAX_VOICES, ModuleId, ModuleInput, ModuleType, NUM_CHANNELS, Router,
        },
        synth_module::{
            InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams, SynthModule, VoiceAlive,
        },
        types::Sample,
    },
    utils::from_ms,
};
use itertools::izip;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct AmplifierConfigChannel {
    level: Sample,
}

impl Default for AmplifierConfigChannel {
    fn default() -> Self {
        Self { level: 1.0 }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AmplifierConfig {
    label: Option<String>,
    channels: [AmplifierConfigChannel; NUM_CHANNELS],
}

pub struct AmplifierUIData {
    pub label: String,
    pub level: StereoSample,
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

struct Channel {
    level: f32,
    voices: [Voice; MAX_VOICES],
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            level: 1.0,
            voices: Default::default(),
        }
    }
}

struct Common {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<AmplifierConfig>,
    input: Buffer,
    level_mod_input: Buffer,
}

pub struct Amplifier {
    common: Common,
    channels: [Channel; NUM_CHANNELS],
}

impl Amplifier {
    pub fn new(id: ModuleId, config: ModuleConfigBox<AmplifierConfig>) -> Self {
        let mut amp = Self {
            common: Common {
                id,
                label: format!("Amplifier {id}"),
                config,
                input: zero_buffer(),
                level_mod_input: zero_buffer(),
            },
            channels: Default::default(),
        };

        {
            let cfg = amp.common.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                amp.common.label = label.clone();
            }

            for (channel, cfg) in amp.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.level = cfg.level;
            }
        }

        amp
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> AmplifierUIData {
        AmplifierUIData {
            label: self.common.label.clone(),
            level: StereoSample::from_iter(self.channels.iter().map(|channel| channel.level)),
        }
    }

    pub fn set_level(&mut self, level: StereoSample) {
        for (chan, value) in self.channels.iter_mut().zip(level.iter()) {
            chan.level = *value;
        }

        {
            let mut cfg = self.common.config.lock();

            for (chan, value) in cfg.channels.iter_mut().zip(level.iter()) {
                chan.level = *value;
            }
        }
    }

    fn process_channel_voice(
        common: &mut Common,
        channel: &mut Channel,
        params: &ProcessParams,
        router: &dyn Router,
        voice_idx: usize,
        channel_idx: usize,
    ) {
        let id = common.id;
        let voice = &mut channel.voices[voice_idx];
        let input = router
            .get_input(
                ModuleInput::new(Input::Audio, id),
                params.samples,
                voice_idx,
                channel_idx,
                &mut common.input,
            )
            .unwrap_or(&ZEROES_BUFFER);
        let level_mod = router
            .get_input(
                ModuleInput::new(Input::Level, id),
                params.samples,
                voice_idx,
                channel_idx,
                &mut common.level_mod_input,
            )
            .unwrap_or(&ONES_BUFFER);

        for (out, input, modulation) in izip!(
            voice.output.iter_mut().take(params.samples),
            input,
            level_mod
        ) {
            *out = input * channel.level * modulation;
        }

        if voice.killed {
            let base = (0.00673795 as Sample).powf((params.sample_rate * from_ms(30.0)).recip());
            let mut sum = 0.0;

            for out in voice.output.iter_mut().take(params.samples) {
                voice.killed_level *= base;
                *out *= voice.killed_level;
                sum += *out * *out;
            }

            voice.killed_output_power =
                (voice.killed_output_power + sum) / (params.samples + 1) as Sample;
        }
    }
}

impl SynthModule for Amplifier {
    fn id(&self) -> ModuleId {
        self.common.id
    }

    fn label(&self) -> String {
        self.common.label.clone()
    }

    fn set_label(&mut self, label: String) {
        self.common.label = label.clone();
        self.common.config.lock().label = Some(label);
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

    fn outputs(&self) -> &'static [DataType] {
        &[DataType::Buffer]
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
        const ALIVE_THRESHOLD: Sample = 0.00000001;

        for voice_alive in alive_state.iter_mut().filter(|alive| alive.killed()) {
            for channel in &self.channels {
                voice_alive.mark_alive(
                    channel.voices[voice_alive.index()].killed_output_power > ALIVE_THRESHOLD,
                );
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                Self::process_channel_voice(
                    &mut self.common,
                    channel,
                    params,
                    router,
                    *voice_idx,
                    channel_idx,
                );
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
    }
}
