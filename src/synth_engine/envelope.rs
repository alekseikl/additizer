use crate::{
    synth_engine::{
        buffer::{Buffer, make_zero_buffer},
        routing::{MAX_VOICES, ModuleId, NUM_CHANNELS, Router},
        synth_module::{NoteOffParams, NoteOnParams, ProcessParams, SynthModule},
    },
    utils::from_ms,
};

#[derive(Default, Clone, Copy)]
pub struct EnvelopeActivityState {
    pub voice_idx: usize,
    pub active: bool,
}

#[derive(Debug)]
struct ReleaseState {
    start_t: f32,
    start_level: f32,
}

struct Voice {
    t: f32,
    attack_from: f32,
    release: Option<ReleaseState>,
    last_level: f32,
    output: Buffer,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            t: 0.0,
            attack_from: 0.0,
            release: None,
            last_level: 0.0,
            output: make_zero_buffer(),
        }
    }

    fn reset(&mut self, same_note_retrigger: bool, sustain: f32) {
        self.t = 0.0;
        self.release = None;
        self.attack_from = if same_note_retrigger {
            self.last_level
        } else {
            0.0
        };
        self.last_level = sustain;
    }

    fn release(&mut self) {
        self.release = Some(ReleaseState {
            start_t: self.t,
            start_level: self.last_level,
        })
    }
}

impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}

struct Channel {
    attack_time: f32,
    decay_time: f32,
    sustain_level: f32,
    release_time: f32,
    voices: [Voice; MAX_VOICES],
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            attack_time: from_ms(10.0),
            decay_time: from_ms(200.0),
            sustain_level: 1.0,
            release_time: from_ms(300.0),
            voices: Default::default(),
        }
    }
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

    pub fn set_attack(&mut self, attack: f32) -> &mut Self {
        for channel in &mut self.channels {
            channel.attack_time = from_ms(attack);
        }
        self
    }

    pub fn set_channel_attack(&mut self, channel: usize, attack: f32) -> &mut Self {
        self.channels[channel].attack_time = from_ms(attack);
        self
    }

    pub fn set_decay(&mut self, decay: f32) -> &mut Self {
        for channel in &mut self.channels {
            channel.decay_time = from_ms(decay);
        }
        self
    }

    pub fn set_channel_decay(&mut self, channel: usize, decay: f32) -> &mut Self {
        self.channels[channel].decay_time = from_ms(decay);
        self
    }

    pub fn set_sustain(&mut self, sustain: f32) -> &mut Self {
        for channel in &mut self.channels {
            channel.sustain_level = sustain;
        }
        self
    }

    pub fn set_channel_sustain(&mut self, channel: usize, sustain: f32) -> &mut Self {
        self.channels[channel].sustain_level = sustain;
        self
    }

    pub fn set_release(&mut self, release: f32) -> &mut Self {
        for channel in &mut self.channels {
            channel.release_time = from_ms(release);
        }
        self
    }

    pub fn set_channel_release(&mut self, channel: usize, release: f32) -> &mut Self {
        self.channels[channel].release_time = from_ms(release);
        self
    }

    pub fn check_activity(&self, activity: &mut [EnvelopeActivityState]) {
        if self.keep_voice_alive {
            for channel in &self.channels {
                for voice_activity in activity.iter_mut() {
                    let voice = &channel.voices[voice_activity.voice_idx];
                    let is_active = if let Some(release) = &voice.release
                        && voice.t - release.start_t >= channel.release_time
                    {
                        false
                    } else {
                        true
                    };
                    voice_activity.active = voice_activity.active || is_active;
                }
            }
        }
    }

    fn process_channel_voice(channel: &mut Channel, params: &ProcessParams, voice_idx: usize) {
        let voice = &mut channel.voices[voice_idx];
        let t_step = 1.0 / params.sample_rate;

        for i in 0..params.samples {
            let out = if let Some(release) = &voice.release {
                let release_t = voice.t - release.start_t;

                if release_t <= channel.release_time {
                    release.start_level * (1.0 - release_t / channel.release_time)
                } else {
                    0.0
                }
            } else if voice.t < channel.attack_time {
                voice.attack_from + (1.0 - voice.attack_from) * (voice.t / channel.attack_time)
            } else if (voice.t - channel.attack_time) < channel.decay_time {
                1.0 - (1.0 - channel.sustain_level)
                    * ((voice.t - channel.attack_time) / channel.decay_time)
            } else {
                voice.last_level
            };

            voice.last_level = out;
            voice.output[i] = out;
            voice.t += t_step;
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
            channel.voices[params.voice_idx]
                .reset(params.same_note_retrigger, channel.sustain_level);
        }
    }

    fn note_off(&mut self, params: &NoteOffParams) {
        for channel in &mut self.channels {
            channel.voices[params.voice_idx].release();
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
