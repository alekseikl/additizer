//! Polynomial approximations and interpolation for SIMD lanes.

use wide::bytemuck::cast;
use wide::f32x4;

use crate::{Smp, WideI32, WideSmp, consts::A4_FREQ};

#[cfg(test)]
mod tests;

/// `2^x` for each lane as an inline polynomial, with no libm call.
///
/// Relative error is below 1e-7 (about half an f32 ulp, on par with `exp2f`)
/// for `x` in `[-125, 125]`; `x` is clamped to that range. Intended for
/// converting whole buffers of pitch values where per-sample `exp2f` calls
/// would dominate.
#[inline(always)]
pub fn fast_exp2_wide(x: WideSmp) -> WideSmp {
    // Minimax fit of 2^f on [-0.5, 0.5], max relative error ≈ 1.9e-9 in exact
    // arithmetic (Remez exchange; see the tests for the f32 bound).
    // Splatted as `const` so they become vector literals rather than
    // runtime array fills.
    const C: [WideSmp; 7] = [
        WideSmp::splat(1.0),
        WideSmp::splat(6.931_472e-1),
        WideSmp::splat(2.402_264_7e-1),
        WideSmp::splat(5.550_328_8e-2),
        WideSmp::splat(9.618_489e-3),
        WideSmp::splat(1.339_993_1e-3),
        WideSmp::splat(1.534_581_2e-4),
    ];
    const MIN_EXP: WideSmp = WideSmp::splat(-125.0);
    const MAX_EXP: WideSmp = WideSmp::splat(125.0);

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
    let scaled: WideI32 = cast::<WideSmp, WideI32>(p) + (xi.trunc_int() << 23);

    cast(scaled)
}

/// [`db_to_gain_fast`](crate::math::db_to_gain_fast) for each lane, built on [`fast_exp2_wide`].
///
/// `10^(dB/20) = 2^(dB * log2(10) / 20)`. Drive values stay well inside the
/// polynomial's accurate range.
#[inline(always)]
pub fn db_to_gain_fast_wide(dbs: WideSmp) -> WideSmp {
    const DB_TO_EXP2: WideSmp = WideSmp::splat(std::f32::consts::LOG2_10 / 20.0);

    fast_exp2_wide(dbs * DB_TO_EXP2)
}

/// [`pitch_to_freq`](crate::math::pitch_to_freq) for each lane, built on [`fast_exp2_wide`].
#[inline(always)]
pub fn pitch_to_freq_wide(pitch: WideSmp) -> WideSmp {
    const A4_FREQ_WIDE: WideSmp = WideSmp::splat(A4_FREQ);

    fast_exp2_wide(pitch) * A4_FREQ_WIDE
}

/// `tan(π x)` for each lane, with `x` in `[0, 0.499]`.
///
/// This is the SVF prewarp `g = tan(π f / sample_rate)`. A degree-6 polynomial
/// covers a quarter turn. Above that, `tan(π x) = 1 / tan(π (1/2 - x))`, so the
/// reduced argument stays exact as `x` approaches one half.
///
/// Relative error against `(x * π).tan()` is below 2e-7 for `x <= 0.25`, and
/// below 5e-5 up to `0.499`, where `tan` is steep and the two f32 arguments
/// differ slightly.
#[inline(always)]
pub(crate) fn fast_tan_pi_wide(ratio: WideSmp) -> WideSmp {
    // Minimax fit of tan(x) / x on [0, π/4], evaluated with f32 `mul_add`.
    const C: [WideSmp; 7] = [
        WideSmp::splat(1.0),
        WideSmp::splat(3.333_307_2e-1),
        WideSmp::splat(1.333_994_9e-1),
        WideSmp::splat(5.334_505_4e-2),
        WideSmp::splat(2.461_384_6e-2),
        WideSmp::splat(2.876_793_7e-3),
        WideSmp::splat(9.508_654_5e-3),
    ];
    const QUARTER: WideSmp = WideSmp::splat(0.25);
    const HALF: WideSmp = WideSmp::splat(0.5);
    const PI_WIDE: WideSmp = WideSmp::splat(std::f32::consts::PI);

    let over = ratio.simd_gt(QUARTER);
    let reduced = over.select(HALF - ratio, ratio);
    let x = reduced * PI_WIDE;
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
        over.select(WideSmp::ONE / tan_x, tan_x)
    } else {
        tan_x
    }
}

/// `sin(π x)` for each lane, with `x` in `[-0.5, 0.5]`.
///
/// This is the raised-cosine ease `0.5 + 0.5 * sin(π (t - 0.5))`. The argument
/// stays inside one half-turn, so one odd polynomial covers both signs.
///
/// Absolute error against `(x * π).sin()` is below 2e-7. The ends are exact:
/// `0` maps to `0` and `±0.5` maps to `±1`.
#[inline(always)]
pub(crate) fn fast_sin_pi_wide(ratio: WideSmp) -> WideSmp {
    // Fit of sin(x) / x on [0, π/2], evaluated with f32 `mul_add`.
    // The constant term is 1, so the approximation is exact at zero.
    const C: [WideSmp; 7] = [
        WideSmp::splat(1.0),
        WideSmp::splat(-0.166_666_67),
        WideSmp::splat(0.008_333_333),
        WideSmp::splat(-0.000_198_412_41),
        WideSmp::splat(2.755_490_6e-6),
        WideSmp::splat(-2.495_599e-8),
        WideSmp::splat(1.440_325_8e-10),
    ];
    const PI_WIDE: WideSmp = WideSmp::splat(std::f32::consts::PI);

    let x = ratio * PI_WIDE;
    let z = x * x;
    let p = C[6]
        .mul_add(z, C[5])
        .mul_add(z, C[4])
        .mul_add(z, C[3])
        .mul_add(z, C[2])
        .mul_add(z, C[1])
        .mul_add(z, C[0]);

    x * p
}

/// Catmull–Rom reads four neighbors, so the kernel stays [`f32x4`].
pub const CATMULL_TAPS: usize = 4;
const _: () = assert!(CATMULL_TAPS == 4);

/// Coefficients of `((b0 * t + b1) * t + b2) * t + b3`, weighting `[s0, s1, s2, s3]`.
const CATMULL_ROM: [f32x4; 4] = [
    f32x4::new([-1.0 / 2.0, 3.0 / 2.0, -3.0 / 2.0, 1.0 / 2.0]),
    f32x4::new([1.0, -5.0 / 2.0, 4.0 / 2.0, -1.0 / 2.0]),
    f32x4::new([-1.0 / 2.0, 0.0 / 2.0, 1.0 / 2.0, 0.0 / 2.0]),
    f32x4::new([0.0 / 2.0, 1.0, 0.0 / 2.0, 0.0 / 2.0]),
];

/// Weights for samples `[s0, s1, s2, s3]` at fractional position `t`.
#[inline(always)]
pub fn catmull_rom(t: Smp) -> f32x4 {
    let [b0, b1, b2, b3] = CATMULL_ROM;
    let t = f32x4::splat(t);

    b0.mul_add(t, b1).mul_add(t, b2).mul_add(t, b3)
}

/// `catmull_rom(t) * g`, evaluated from the pre-multiplied powers
/// `g`, `g * t`, `g * t²`, `g * t³`.
///
/// Useful when `t` and `g` for several taps are already held in SIMD lanes:
/// the powers can be formed once for all lanes, and each lane's weights then
/// cost one multiply plus three FMAs with a broadcast lane operand, with no
/// copies of the basis constants.
#[inline(always)]
pub fn catmull_rom_from_powers(g: Smp, gt: Smp, gt2: Smp, gt3: Smp) -> f32x4 {
    let [b0, b1, b2, b3] = CATMULL_ROM;

    let w = b3 * f32x4::splat(g);
    let w = b2.mul_add(f32x4::splat(gt), w);
    let w = b1.mul_add(f32x4::splat(gt2), w);

    b0.mul_add(f32x4::splat(gt3), w)
}
