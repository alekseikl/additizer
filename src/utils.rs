use crate::synth_engine::Sample;

const ST_TO_OCTAVE_MULT: Sample = 12.0f32.recip();

macro_rules! log {
    ($($args:tt)*) => {
        ::nih_plug::nih_log!($($args)*)
    };
}
pub(crate) use log;

pub const MINUS_INFINITY_DB: f32 = nih_plug::util::MINUS_INFINITY_DB;

#[inline]
pub fn db_to_gain(dbs: f32) -> f32 {
    nih_plug::util::db_to_gain(dbs)
}

#[inline]
pub fn gain_to_db(gain: f32) -> f32 {
    nih_plug::util::gain_to_db(gain)
}

#[inline]
pub fn db_to_gain_fast(dbs: f32) -> f32 {
    nih_plug::util::db_to_gain_fast(dbs)
}

#[inline]
pub fn gain_to_db_fast(gain: f32) -> f32 {
    nih_plug::util::gain_to_db_fast(gain)
}

#[inline]
pub const fn from_ms(ms: f32) -> f32 {
    ms * 0.001
}

#[inline(always)]
pub const fn note_to_pitch(note: Sample) -> Sample {
    (note - 69.0) / 12.0
}

// Pitch in octave units
#[inline(always)]
pub fn pitch_to_freq(pitch: Sample) -> Sample {
    pitch.exp2() * 440.0
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

    pub fn matches(&self, idx: usize) -> bool {
        let i = idx as isize + 1;
        let result = if self.mul == 0 {
            i == self.add
        } else {
            let scaled = (i - self.add) as f32 / self.mul as f32;

            scaled >= 0.0 && scaled.fract().abs() < f32::EPSILON
        };

        result ^ self.inverted
    }
}
