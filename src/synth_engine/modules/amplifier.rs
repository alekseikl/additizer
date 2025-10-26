use crate::synth_engine::{
    buffer::{Buffer, ONES_BUFFER, ZEROES_BUFFER, make_zero_buffer},
    routing::{MAX_VOICES, ModuleId, ModuleInput, NUM_CHANNELS, Router},
    synth_module::{
        BufferOutputModule, ModuleConfig, NoteOffParams, NoteOnParams, ProcessParams, SynthModule,
    },
    types::{Sample, StereoValue},
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
    channels: [AmplifierConfigChannel; NUM_CHANNELS],
}

struct Voice {
    output: Buffer,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            output: make_zero_buffer(),
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
    config: ModuleConfig<AmplifierConfig>,
    input: Buffer,
    level_mod_input: Buffer,
}

pub struct Amplifier {
    common: Common,
    channels: [Channel; NUM_CHANNELS],
}

impl Amplifier {
    pub fn new(config: ModuleConfig<AmplifierConfig>) -> Self {
        let mut amp = Self {
            common: Common {
                config,
                input: make_zero_buffer(),
                level_mod_input: make_zero_buffer(),
            },
            channels: Default::default(),
        };

        amp.common.config.access(|cfg| {
            for (channel, cfg) in amp.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.level = cfg.level;
            }
        });

        amp
    }

    pub fn set_level(&mut self, level: StereoValue) {
        for (chan, value) in self.channels.iter_mut().zip(level.iter()) {
            chan.level = value;
        }

        self.common.config.access(|cfg| {
            for (chan, value) in cfg.channels.iter_mut().zip(level.iter()) {
                chan.level = value;
            }
        });
    }

    fn process_channel_voice(
        common: &mut Common,
        channel: &mut Channel,
        params: &ProcessParams,
        router: &dyn Router,
        voice_idx: usize,
        channel_idx: usize,
    ) {
        let voice = &mut channel.voices[voice_idx];
        let input = router
            .get_input(
                ModuleInput::AmplifierInput(common.config.id()),
                voice_idx,
                channel_idx,
                &mut common.input,
            )
            .unwrap_or(&ZEROES_BUFFER);
        let level_mod = router
            .get_input(
                ModuleInput::AmplifierLevel(common.config.id()),
                voice_idx,
                channel_idx,
                &mut common.level_mod_input,
            )
            .unwrap_or(&ONES_BUFFER);

        for (out, input, modulation, _) in
            izip!(&mut voice.output, input, level_mod, 0..params.samples)
        {
            *out = input * channel.level * modulation;
        }
    }
}

impl SynthModule for Amplifier {
    fn get_id(&self) -> ModuleId {
        self.common.config.id()
    }

    fn note_on(&mut self, _: &NoteOnParams) {}
    fn note_off(&mut self, _: &NoteOffParams) {}

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
}

impl BufferOutputModule for Amplifier {
    fn get_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
    }
}
