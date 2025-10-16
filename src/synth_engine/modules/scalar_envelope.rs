use crate::{
    synth_engine::{
        envelope::{self, EnvelopeActivityState, EnvelopeChannel, EnvelopeVoice},
        routing::{MAX_VOICES, ModuleId, NUM_CHANNELS, Router},
        synth_module::{
            NoteOffParams, NoteOnParams, ProcessParams, ScalarOutputModule, ScalarOutputs,
            SynthModule,
        },
        types::{Sample, StereoValue},
    },
    utils::from_ms,
};

struct Voice {
    env: EnvelopeVoice,
    needs_reset: bool,
    first_output: Sample,
    output: Sample,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            env: EnvelopeVoice::default(),
            needs_reset: true,
            first_output: 0.0,
            output: 0.0,
        }
    }
}

#[derive(Default)]
struct Channel {
    env: EnvelopeChannel,
    voices: [Voice; MAX_VOICES],
}

pub struct ScalarEnvelope {
    module_id: ModuleId,
    keep_voice_alive: bool,
    channels: [Channel; NUM_CHANNELS],
}

impl ScalarEnvelope {
    pub fn new() -> Self {
        Self {
            module_id: 0,
            keep_voice_alive: false,
            channels: Default::default(),
        }
    }

    pub fn set_id(&mut self, module_id: ModuleId) {
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
}

impl SynthModule for ScalarEnvelope {
    fn get_id(&self) -> ModuleId {
        self.module_id
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            let voice = &mut channel.voices[params.voice_idx];

            envelope::reset_voice(&channel.env, &mut voice.env, params.same_note_retrigger);

            if !params.same_note_retrigger {
                voice.needs_reset = true;
            }
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
                let voice = &mut channel.voices[*voice_idx];

                if voice.needs_reset {
                    voice.first_output =
                        envelope::process_voice(&channel.env, &mut voice.env, params.buffer_t_step);
                    voice.needs_reset = false;
                }

                voice.output =
                    envelope::process_voice(&channel.env, &mut voice.env, params.buffer_t_step);
            }
        }
    }
}

impl ScalarOutputModule for ScalarEnvelope {
    fn get_output(&self, voice_idx: usize, channel: usize) -> ScalarOutputs {
        let voice = &self.channels[channel].voices[voice_idx];

        ScalarOutputs {
            first: voice.first_output,
            current: voice.output,
        }
    }
}
