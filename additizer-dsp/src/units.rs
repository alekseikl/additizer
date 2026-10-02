use crate::Sample;

/// MIDI note number for middle C (C4).
pub const C4_NOTE: u8 = 60;
/// Pitch of C4 in octave units (relative to A4).
pub const C4_PITCH: Sample = note_to_pitch(C4_NOTE as Sample);
/// Lowest cutoff, in octave units (same space as pitch).
pub const MIN_CUTOFF: Sample = -4.0;
/// Highest cutoff, in octave units (same space as pitch).
pub const MAX_CUTOFF: Sample = 7.0;

const A4_FREQ: Sample = 440.0;
const ST_TO_OCTAVE_MULT: Sample = 12.0f32.recip();

/// Gain at or below this level is treated as silence.
const MINUS_INFINITY_DB: Sample = -100.0;
/// Amplitude equal to [`MINUS_INFINITY_DB`].
const MINUS_INFINITY_GAIN: Sample = 1e-5;

/// Convert decibels to a voltage gain ratio, treating anything below -100 dB as silence.
#[inline]
pub fn db_to_gain(dbs: f32) -> f32 {
    if dbs > MINUS_INFINITY_DB {
        10.0f32.powf(dbs * 0.05)
    } else {
        0.0
    }
}

/// Convert a voltage gain ratio to decibels. Non-positive ratios become -100 dB.
#[inline]
pub fn gain_to_db(gain: f32) -> f32 {
    f32::max(gain, MINUS_INFINITY_GAIN).log10() * 20.0
}

/// Approximation of [`db_to_gain`] using `exp()`. Does not clamp below -100 dB to zero.
#[inline]
pub fn db_to_gain_fast(dbs: f32) -> f32 {
    const CONVERSION_FACTOR: f32 = std::f32::consts::LN_10 / 20.0;
    (dbs * CONVERSION_FACTOR).exp()
}

/// Approximation of [`gain_to_db`] using `ln()`.
#[inline]
pub fn gain_to_db_fast(gain: f32) -> f32 {
    const CONVERSION_FACTOR: f32 = std::f32::consts::LOG10_E * 20.0;
    f32::max(gain, MINUS_INFINITY_GAIN).ln() * CONVERSION_FACTOR
}

#[inline]
pub const fn from_ms(ms: f32) -> f32 {
    ms * 0.001
}

#[inline(always)]
pub const fn note_to_pitch(note: Sample) -> Sample {
    (note - 69.0) * ST_TO_OCTAVE_MULT
}

/// Pitch in octave units.
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
        ((power * value).exp() - 1.0) / (power.exp() - 1.0)
    }
}

/// Constant-power pan law: `pan` in [-1, 1] → per-channel gain.
#[inline(always)]
pub fn pan_gain(pan: Sample, channel_idx: usize) -> Sample {
    let t = 1.0 + pan * (2.0 * channel_idx as Sample - 1.0);
    (t * std::f32::consts::FRAC_PI_4).sin() * std::f32::consts::SQRT_2
}
