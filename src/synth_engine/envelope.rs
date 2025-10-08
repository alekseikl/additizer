use crate::{
    synth_engine::{
        buffer::{Buffer, make_zero_buffer},
        routing::{MAX_VOICES, ModuleId, Router},
        synth_module::{NoteOffParams, NoteOnParams, ProcessParams, SynthModule},
    },
    utils::from_ms,
};

pub struct EnvelopeActivityState {
    pub voice_idx: usize,
    pub active: bool,
}

#[derive(Debug)]
struct ReleaseState {
    start_t: f32,
    start_level: f32,
}

struct EnvelopeVoice {
    t: f32,
    attack_from: f32,
    release: Option<ReleaseState>,
    last_level: f32,
    output: Buffer,
}

impl EnvelopeVoice {
    pub fn new() -> Self {
        Self {
            t: 0.0,
            attack_from: 0.0,
            release: None,
            last_level: 0.0,
            output: make_zero_buffer(),
        }
    }

    fn reset(&mut self, same_note_retrigger: bool) {
        self.t = 0.0;
        self.release = None;

        if same_note_retrigger {
            self.attack_from = self.last_level;
        } else {
            self.attack_from = 0.0;
            self.last_level = 0.0;
        }
    }

    fn release(&mut self) {
        self.release = Some(ReleaseState {
            start_t: self.t,
            start_level: self.last_level,
        })
    }
}

impl Default for EnvelopeVoice {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EnvelopeModule {
    module_id: ModuleId,
    attack_time: f32,
    decay_time: f32,
    sustain_level: f32,
    release_time: f32,
    voices: [EnvelopeVoice; MAX_VOICES],
}

impl EnvelopeModule {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            attack_time: from_ms(10.0),
            decay_time: from_ms(200.0),
            sustain_level: 1.0,
            release_time: from_ms(300.0),
            voices: Default::default(),
        }
    }

    pub fn check_activity(&self, activity: &mut [EnvelopeActivityState]) {
        for voice_activity in activity {
            let voice = &self.voices[voice_activity.voice_idx];
            let is_active = if let Some(release) = &voice.release
                && voice.t - release.start_t >= self.release_time
            {
                false
            } else {
                true
            };
            voice_activity.active = voice_activity.active || is_active;
        }
    }

    fn process_voice(&mut self, params: &ProcessParams, _: &dyn Router, voice_idx: usize) {
        let voice = &mut self.voices[voice_idx];
        let t_step = 1.0 / params.sample_rate;

        for i in 0..params.samples {
            let out = if let Some(release) = &voice.release {
                let release_t = voice.t - release.start_t;

                if release_t <= self.release_time {
                    release.start_level * (1.0 - release_t / self.release_time)
                } else {
                    0.0
                }
            } else if voice.t < self.attack_time {
                voice.attack_from + (1.0 - voice.attack_from) * (voice.t / self.attack_time)
            } else if voice.t < self.decay_time {
                1.0 - (1.0 - self.sustain_level) * ((voice.t - self.attack_time) / self.decay_time)
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

    fn get_output(&self, voice_idx: usize) -> &Buffer {
        &self.voices[voice_idx].output
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        self.voices[params.voice_idx].reset(params.same_note_retrigger);
    }

    fn note_off(&mut self, params: &NoteOffParams) {
        self.voices[params.voice_idx].release();
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for voice_idx in params.active_voices {
            self.process_voice(params, router, *voice_idx);
        }
    }
}
