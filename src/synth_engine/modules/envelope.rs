use std::any::Any;

use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    envelope::{self, EnvelopeActivityState, EnvelopeChannel, EnvelopeVoice},
    routing::{InputType, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, OutputType, Router},
    synth_module::{ModuleConfigBox, NoteOffParams, NoteOnParams, ProcessParams, SynthModule},
    types::{Sample, StereoSample},
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

pub struct EnvelopeUI {
    pub attack: StereoSample,
    pub decay: StereoSample,
    pub sustain: StereoSample,
    pub release: StereoSample,
    pub keep_voice_alive: bool,
}

struct Voice {
    env: EnvelopeVoice,
    prev_output: Sample,
    output: Sample,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            env: EnvelopeVoice::default(),
            prev_output: 0.0,
            output: 0.0,
        }
    }
}

#[derive(Default)]
struct Channel {
    env: EnvelopeChannel,
    voices: [Voice; MAX_VOICES],
}

pub struct Envelope {
    id: ModuleId,
    config: ModuleConfigBox<EnvelopeConfig>,
    keep_voice_alive: bool,
    channels: [Channel; NUM_CHANNELS],
}

macro_rules! set_param_method {
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) -> &mut Self {
            for (channel, $param) in self.channels.iter_mut().zip($param.iter()) {
                channel.env.$param = $transform;
            }

            {
                let mut cfg = self.config.lock();

                for (cfg_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                    cfg_channel.$param = channel.env.$param;
                }
            }

            self
        }
    };
}

macro_rules! extract_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channels.iter().map(|channel| channel.env.$param))
    };
}

impl Envelope {
    pub fn new(id: ModuleId, config: ModuleConfigBox<EnvelopeConfig>) -> Self {
        let mut env = Self {
            id,
            config,
            keep_voice_alive: true,
            channels: Default::default(),
        };

        {
            let cfg = env.config.lock();
            for (channel, cfg_channel) in env.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.env = cfg_channel.clone();
            }
            env.keep_voice_alive = cfg.keep_voice_alive;
        }

        env
    }

    gen_downcast_methods!(Envelope);

    pub fn get_ui(&self) -> EnvelopeUI {
        EnvelopeUI {
            attack: extract_param!(self, attack),
            decay: extract_param!(self, decay),
            sustain: extract_param!(self, sustain),
            release: extract_param!(self, release),
            keep_voice_alive: self.keep_voice_alive,
        }
    }

    pub fn set_keep_voice_alive(&mut self, keep_alive: bool) -> &mut Self {
        self.keep_voice_alive = keep_alive;

        {
            let mut cfg = self.config.lock();
            cfg.keep_voice_alive = keep_alive;
        }

        self
    }

    set_param_method!(set_attack, attack, *attack);
    set_param_method!(set_decay, decay, *decay);
    set_param_method!(set_sustain, sustain, *sustain);
    set_param_method!(set_release, release, *release);

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

        voice.prev_output = voice.output;
        voice.output = envelope::process_voice_sample(&channel.env, &mut voice.env);
        envelope::advance_voice(&mut voice.env, params.buffer_t_step, voice.output);
    }
}

impl SynthModule for Envelope {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Envelope
    }

    fn is_spectral_rate(&self) -> bool {
        true
    }

    fn inputs(&self) -> &'static [InputType] {
        &[]
    }

    fn output_type(&self) -> OutputType {
        OutputType::Scalar
    }

    fn note_on(&mut self, params: &NoteOnParams, _router: &dyn Router) {
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

    fn get_scalar_output(&self, voice_idx: usize, channel: usize) -> (Sample, Sample) {
        let voice = &self.channels[channel].voices[voice_idx];

        (voice.prev_output, voice.output)
    }
}
