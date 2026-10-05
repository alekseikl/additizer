use wide::{bytemuck::cast, f32x4, i32x4};

use crate::Sample;

#[cfg(test)]
mod tests;

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

/// `2^x` for four lanes as an inline polynomial, with no libm call.
///
/// Relative error is below 1e-7 (about half an f32 ulp, on par with `exp2f`)
/// for `x` in `[-125, 125]`; `x` is clamped to that range. Intended for
/// converting whole buffers of pitch values where per-sample `exp2f` calls
/// would dominate.
#[inline(always)]
pub fn fast_exp2_x4(x: f32x4) -> f32x4 {
    // Minimax fit of 2^f on [-0.5, 0.5], max relative error ≈ 1.9e-9 in exact
    // arithmetic (Remez exchange; see the tests for the f32 bound).
    // Splatted as `const` so they become vector literals rather than
    // runtime array fills.
    const C: [f32x4; 7] = [
        f32x4::splat(1.0),
        f32x4::splat(6.931_472e-1),
        f32x4::splat(2.402_264_7e-1),
        f32x4::splat(5.550_328_8e-2),
        f32x4::splat(9.618_489e-3),
        f32x4::splat(1.339_993_1e-3),
        f32x4::splat(1.534_581_2e-4),
    ];
    const MIN_EXP: f32x4 = f32x4::splat(-125.0);
    const MAX_EXP: f32x4 = f32x4::splat(125.0);

    let x = x.fast_max(MIN_EXP).fast_min(MAX_EXP);
    let xi = x.round();
    let f = x - xi;

    let p = C[6]
        .mul_add(f, C[5])
        .mul_add(f, C[4])
        .mul_add(f, C[3])
        .mul_add(f, C[2])
        .mul_add(f, C[1])
        .mul_add(f, C[0]);

    // `p` is in [2^-0.5, 2^0.5]; scaling by 2^xi is an exact add to its
    // exponent field, which cannot overflow or denormalize within `MAX_EXP`.
    let scaled: i32x4 = cast::<f32x4, i32x4>(p) + (xi.trunc_int() << 23);

    cast(scaled)
}

/// [`pitch_to_freq`] for four lanes, built on [`fast_exp2_x4`].
#[inline(always)]
pub fn fast_pitch_to_freq_x4(pitch: f32x4) -> f32x4 {
    const A4_FREQ_X4: f32x4 = f32x4::splat(A4_FREQ);

    fast_exp2_x4(pitch) * A4_FREQ_X4
}

/// `tan(π x)` for four lanes, with `x` in `[0, 0.499]`.
///
/// This is the SVF prewarp `g = tan(π f / sample_rate)`. A degree-6 polynomial
/// covers a quarter turn. Above that, `tan(π x) = 1 / tan(π (1/2 - x))`, so the
/// reduced argument stays exact as `x` approaches one half.
///
/// Relative error against `(x * π).tan()` is below 2e-7 for `x <= 0.25`, and
/// below 5e-5 up to `0.499`, where `tan` is steep and the two f32 arguments
/// differ slightly.
#[inline(always)]
pub(crate) fn fast_tan_pi_x4(ratio: f32x4) -> f32x4 {
    // Minimax fit of tan(x) / x on [0, π/4], evaluated with f32 `mul_add`.
    const C: [f32x4; 7] = [
        f32x4::splat(1.0),
        f32x4::splat(3.333_307_2e-1),
        f32x4::splat(1.333_994_9e-1),
        f32x4::splat(5.334_505_4e-2),
        f32x4::splat(2.461_384_6e-2),
        f32x4::splat(2.876_793_7e-3),
        f32x4::splat(9.508_654_5e-3),
    ];
    const QUARTER: f32x4 = f32x4::splat(0.25);
    const HALF: f32x4 = f32x4::splat(0.5);
    const PI_X4: f32x4 = f32x4::splat(std::f32::consts::PI);

    let over = ratio.simd_gt(QUARTER);
    let reduced = over.select(HALF - ratio, ratio);
    let x = reduced * PI_X4;
    let t = x * x;
    let p = C[6]
        .mul_add(t, C[5])
        .mul_add(t, C[4])
        .mul_add(t, C[3])
        .mul_add(t, C[2])
        .mul_add(t, C[1])
        .mul_add(t, C[0]);
    let tan_x = x * p;

    if over.any() {
        over.select(f32x4::ONE / tan_x, tan_x)
    } else {
        tan_x
    }
}

#[inline(always)]
pub fn freq_to_pitch(freq: Sample) -> Sample {
    (freq * A4_FREQ.recip()).log2()
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
