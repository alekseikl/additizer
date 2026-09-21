use crate::synth_engine::Sample;

/// Silence floor for dB level parameters. At or below this, gain is treated as zero.
pub const MIN_LEVEL_DB: Sample = -60.0;
/// Upper clamp for dB level parameters.
pub const MAX_LEVEL_DB: Sample = 24.0;
/// MIDI note number for middle C (C4).
pub const C4_NOTE: u8 = 60;
/// Pitch of C4 in octave units (relative to A4).
pub const C4_PITCH: Sample = note_to_pitch(C4_NOTE as Sample);
const A4_FREQ: Sample = 440.0;

const ST_TO_OCTAVE_MULT: Sample = 12.0f32.recip();

macro_rules! log {
    ($($args:tt)*) => {
        ::nice_plug::nice_log!($($args)*)
    };
}
pub(crate) use log;

#[inline]
pub fn db_to_gain(dbs: f32) -> f32 {
    nice_plug::util::db_to_gain(dbs)
}

#[inline]
pub fn gain_to_db(gain: f32) -> f32 {
    nice_plug::util::gain_to_db(gain)
}

#[inline]
pub fn db_to_gain_fast(dbs: f32) -> f32 {
    nice_plug::util::db_to_gain_fast(dbs)
}

#[inline]
pub fn gain_to_db_fast(gain: f32) -> f32 {
    nice_plug::util::gain_to_db_fast(gain)
}

#[inline]
pub const fn from_ms(ms: f32) -> f32 {
    ms * 0.001
}

#[inline(always)]
pub const fn note_to_pitch(note: Sample) -> Sample {
    (note - 69.0) * ST_TO_OCTAVE_MULT
}

// Pitch in octave units
#[inline(always)]
pub fn pitch_to_freq(pitch: Sample) -> Sample {
    pitch.exp2() * A4_FREQ
}

#[inline(always)]
pub fn freq_to_pitch(freq: Sample) -> Sample {
    (freq / A4_FREQ).log2()
}

#[inline(always)]
pub fn freq_to_c4_pitch(freq: Sample) -> Sample {
    freq_to_pitch(freq) - C4_PITCH
}

#[inline(always)]
pub const fn from_st(st: Sample) -> Sample {
    st * ST_TO_OCTAVE_MULT
}

#[inline(always)]
pub fn power_scale(value: Sample, power: Sample) -> Sample {
    if power.abs() < 0.005 {
        value
    } else {
        ((power * value).exp() - 1.0) / ((power).exp() - 1.0)
    }
}

/// Constant-power pan law: `pan` in [-1, 1] → per-channel gain.
#[inline(always)]
pub fn pan_gain(pan: Sample, channel_idx: usize) -> Sample {
    let t = 1.0 + pan * (2.0 * channel_idx as Sample - 1.0);
    (t * std::f32::consts::FRAC_PI_4).sin() * std::f32::consts::SQRT_2
}

pub struct NthElement {
    mul: isize,
    add: isize,
    inverted: bool,
}

impl NthElement {
    pub fn new(mul: isize, add: isize, inverted: bool) -> Self {
        Self { mul, add, inverted }
    }

    pub fn matches(&self, harmonic: usize) -> bool {
        let harmonic = harmonic as isize;
        let result = if self.mul == 0 {
            harmonic == self.add
        } else {
            let diff = harmonic - self.add;
            diff % self.mul == 0 && diff / self.mul >= 0
        };

        result ^ self.inverted
    }
}
