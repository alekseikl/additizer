use crate::{
    synth_engine::{
        buffer::{Buffer, make_zero_buffer},
        envelope::{self, EnvelopeActivityState, EnvelopeChannel, EnvelopeVoice},
        routing::{MAX_VOICES, ModuleId, NUM_CHANNELS, Router},
        synth_module::{NoteOffParams, NoteOnParams, ProcessParams, SynthModule},
        types::StereoValue,
    },
    utils::from_ms,
};

struct Voice {
    env: EnvelopeVoice,
    output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            env: EnvelopeVoice::default(),
            output: make_zero_buffer(),
        }
    }
}

#[derive(Default)]
struct Channel {
    env: EnvelopeChannel,
    voices: [Voice; MAX_VOICES],
}

pub struct EnvelopeModule {
    module_id: ModuleId,
    keep_voice_alive: bool,
    channels: [Channel; NUM_CHANNELS],
}

impl EnvelopeModule {
    pub fn new() -> Self {
        Self {
            module_id: 0,
            keep_voice_alive: true,
            channels: Default::default(),
        }
    }

    pub(super) fn set_id(&mut self, module_id: ModuleId) {
        self.module_id = module_id;
    }

    pub fn set_keep_voice_alive(&mut self, keep_alive: bool) -> &mut Self {
        self.keep_voice_alive = keep_alive;
        self
    }

    pub fn set_attack(&mut self, attack: StereoValue) -> &mut Self {
        for (channel, attack) in self.channels.iter_mut().zip(attack.iter()) {
            channel.env.attack_time = from_ms(attack);
        }
        self
    }

    pub fn set_decay(&mut self, decay: StereoValue) -> &mut Self {
        for (channel, decay) in self.channels.iter_mut().zip(decay.iter()) {
            channel.env.decay_time = from_ms(decay);
        }
        self
    }

    pub fn set_sustain(&mut self, sustain: StereoValue) -> &mut Self {
        for (channel, sustain) in self.channels.iter_mut().zip(sustain.iter()) {
            channel.env.sustain_level = sustain;
        }
        self
    }

    pub fn set_release(&mut self, release: StereoValue) -> &mut Self {
        for (channel, release) in self.channels.iter_mut().zip(release.iter()) {
            channel.env.release_time = from_ms(release);
        }
        self
    }

    pub fn set_channel_release(&mut self, channel: usize, release: f32) -> &mut Self {
        self.channels[channel].env.release_time = from_ms(release);
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
        let t_step = 1.0 / params.sample_rate;

        for (out, _) in voice.output.iter_mut().zip(0..params.samples) {
            *out = envelope::process_voice(&channel.env, &mut voice.env, t_step);
        }
    }
}

impl SynthModule for EnvelopeModule {
    fn get_id(&self) -> ModuleId {
        self.module_id
    }

    fn get_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
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
