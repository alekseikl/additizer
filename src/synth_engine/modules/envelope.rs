use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        buffer::{Buffer, make_zero_buffer},
        envelope::{self, EnvelopeActivityState, EnvelopeChannel, EnvelopeVoice},
        routing::{MAX_VOICES, ModuleId, NUM_CHANNELS, Router},
        synth_module::{
            BufferOutputModule, ModuleConfig, NoteOffParams, NoteOnParams, ProcessParams,
            ScalarOutputModule, ScalarOutputs, SynthModule,
        },
        types::{Sample, StereoSample},
    },
    utils::from_ms,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct EnvelopeConfig {
    keep_voice_alive: bool,
    channels: [EnvelopeChannel; NUM_CHANNELS],
}

impl Default for EnvelopeConfig {
    fn default() -> Self {
        Self {
            keep_voice_alive: true,
            channels: Default::default(),
        }
    }
}

struct Voice {
    env: EnvelopeVoice,
    next_output_sample: Sample,
    output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            env: EnvelopeVoice::default(),
            next_output_sample: 0.0,
            output: make_zero_buffer(),
        }
    }
}

#[derive(Default)]
struct Channel {
    env: EnvelopeChannel,
    voices: [Voice; MAX_VOICES],
}

pub struct Envelope {
    config: ModuleConfig<EnvelopeConfig>,
    keep_voice_alive: bool,
    channels: [Channel; NUM_CHANNELS],
}

impl Envelope {
    pub fn new(config: ModuleConfig<EnvelopeConfig>) -> Self {
        let mut env = Self {
            config,
            keep_voice_alive: true,
            channels: Default::default(),
        };

        env.config.access(|cfg| {
            for (channel, cfg_channel) in env.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.env = cfg_channel.clone();
            }
            env.keep_voice_alive = cfg.keep_voice_alive;
        });

        env
    }

    pub fn set_keep_voice_alive(&mut self, keep_alive: bool) -> &mut Self {
        self.keep_voice_alive = keep_alive;

        self.config.access(|cfg| {
            cfg.keep_voice_alive = keep_alive;
        });

        self
    }

    pub fn set_attack(&mut self, attack: StereoSample) -> &mut Self {
        for (channel, attack) in self.channels.iter_mut().zip(attack.iter()) {
            channel.env.attack_time = from_ms(*attack);
        }

        self.config.access(|cfg| {
            for (cfg_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                cfg_channel.attack_time = channel.env.attack_time;
            }
        });

        self
    }

    pub fn set_decay(&mut self, decay: StereoSample) -> &mut Self {
        for (channel, decay) in self.channels.iter_mut().zip(decay.iter()) {
            channel.env.decay_time = from_ms(*decay);
        }

        self.config.access(|cfg| {
            for (cfg_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                cfg_channel.decay_time = channel.env.decay_time;
            }
        });

        self
    }

    pub fn set_sustain(&mut self, sustain: StereoSample) -> &mut Self {
        for (channel, sustain) in self.channels.iter_mut().zip(sustain.iter()) {
            channel.env.sustain_level = *sustain;
        }

        self.config.access(|cfg| {
            for (cfg_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                cfg_channel.sustain_level = channel.env.sustain_level;
            }
        });

        self
    }

    pub fn set_release(&mut self, release: StereoSample) -> &mut Self {
        for (channel, release) in self.channels.iter_mut().zip(release.iter()) {
            channel.env.release_time = from_ms(*release);
        }

        self.config.access(|cfg| {
            for (cfg_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                cfg_channel.release_time = channel.env.release_time;
            }
        });

        self
    }

    pub fn check_activity(&self, activity: &mut [EnvelopeActivityState]) {
        if self.keep_voice_alive {
            for channel in &self.channels {
                for voice_activity in activity.iter_mut() {
                    let voice = &channel.voices[voice_activity.voice_idx];

                    voice_activity.active = voice_activity.active
                        || envelope::is_voice_active(&channel.env, &voice.env);
                }
            }
        }
    }

    fn process_channel_voice(channel: &mut Channel, params: &ProcessParams, voice_idx: usize) {
        let voice = &mut channel.voices[voice_idx];

        for (out, _) in voice.output.iter_mut().zip(0..params.samples) {
            let value = envelope::process_voice_sample(&channel.env, &mut voice.env);

            *out = value;
            envelope::advance_voice(&mut voice.env, params.t_step, value);
        }

        voice.next_output_sample = envelope::process_voice_sample(&channel.env, &mut voice.env);
    }
}

impl SynthModule for Envelope {
    fn get_id(&self) -> ModuleId {
        self.config.id()
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            envelope::reset_voice(
                &channel.env,
                &mut channel.voices[params.voice_idx].env,
                params.same_note_retrigger,
            );
        }
    }

    fn note_off(&mut self, params: &NoteOffParams) {
        for channel in &mut self.channels {
            envelope::release_voice(&mut channel.voices[params.voice_idx].env);
        }
    }

    fn process(&mut self, params: &ProcessParams, _router: &dyn Router) {
        for channel in &mut self.channels {
            for voice_idx in params.active_voices {
                Self::process_channel_voice(channel, params, *voice_idx);
            }
        }
    }
}

impl BufferOutputModule for Envelope {
    fn get_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
    }
}

impl ScalarOutputModule for Envelope {
    fn get_output(&self, voice_idx: usize, channel: usize) -> ScalarOutputs {
        let voice = &self.channels[channel].voices[voice_idx];

        ScalarOutputs {
            first: voice.output[0],
            current: voice.next_output_sample,
        }
    }
}
