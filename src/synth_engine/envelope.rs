use serde::{Deserialize, Serialize};

use crate::{synth_engine::types::Sample, utils::from_ms};

#[derive(Debug)]
pub struct EnvelopeActivityState {
    pub voice_idx: usize,
    pub active: bool,
}

#[derive(Debug)]
pub struct ReleaseState {
    release_t: Sample,
    from_level: Sample,
}

#[derive(Debug)]
pub struct EnvelopeVoice {
    t: Sample,
    attack_from: Sample,
    release: Option<ReleaseState>,
    last_level: Sample,
}

impl Default for EnvelopeVoice {
    fn default() -> Self {
        Self {
            t: 0.0,
            attack_from: 0.0,
            release: None,
            last_level: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeChannel {
    pub attack_time: Sample,
    pub decay_time: Sample,
    pub sustain_level: Sample,
    pub release_time: Sample,
}

impl Default for EnvelopeChannel {
    fn default() -> Self {
        Self {
            attack_time: from_ms(10.0),
            decay_time: from_ms(200.0),
            sustain_level: 1.0,
            release_time: from_ms(300.0),
        }
    }
}

#[inline]
pub fn reset_voice(
    channel: &EnvelopeChannel,
    voice: &mut EnvelopeVoice,
    same_note_retrigger: bool,
) {
    voice.t = 0.0;
    voice.release = None;
    voice.attack_from = if same_note_retrigger {
        voice.last_level
    } else {
        0.0
    };
    voice.last_level = channel.sustain_level;
}

#[inline]
pub fn release_voice(voice: &mut EnvelopeVoice) {
    voice.release = Some(ReleaseState {
        release_t: voice.t,
        from_level: voice.last_level,
    })
}

#[inline]
pub fn is_voice_active(channel: &EnvelopeChannel, voice: &EnvelopeVoice) -> bool {
    if let Some(release) = &voice.release
        && voice.t - release.release_t >= channel.release_time
    {
        false
    } else {
        true
    }
}

#[inline(always)]
pub fn process_voice_sample(channel: &EnvelopeChannel, voice: &mut EnvelopeVoice) -> Sample {
    if let Some(release) = &voice.release {
        let release_t = voice.t - release.release_t;

        if release_t <= channel.release_time {
            release.from_level * (1.0 - release_t / channel.release_time)
        } else {
            0.0
        }
    } else if voice.t < channel.attack_time {
        voice.attack_from + (1.0 - voice.attack_from) * (voice.t / channel.attack_time)
    } else if (voice.t - channel.attack_time) < channel.decay_time {
        1.0 - (1.0 - channel.sustain_level) * ((voice.t - channel.attack_time) / channel.decay_time)
    } else {
        voice.last_level
    }
}

#[inline(always)]
pub fn advance_voice(voice: &mut EnvelopeVoice, t_step: Sample, last_level: Sample) {
    voice.last_level = last_level;
    voice.t += t_step;
}

#[inline(always)]
pub fn process_voice(
    channel: &EnvelopeChannel,
    voice: &mut EnvelopeVoice,
    t_step: Sample,
) -> Sample {
    let out = process_voice_sample(channel, voice);

    advance_voice(voice, t_step, out);
    out
}
