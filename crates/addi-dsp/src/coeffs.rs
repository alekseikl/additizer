//! Catmull-Rom cubic basis for four-sample interpolation.

use wide::f32x4;

use crate::Sample;

/// Coefficients of `((b0 * t + b1) * t + b2) * t + b3`, weighting `[s0, s1, s2, s3]`.
const CATMULL_ROM: [f32x4; 4] = [
    f32x4::new([-1.0 / 2.0, 3.0 / 2.0, -3.0 / 2.0, 1.0 / 2.0]),
    f32x4::new([1.0, -5.0 / 2.0, 4.0 / 2.0, -1.0 / 2.0]),
    f32x4::new([-1.0 / 2.0, 0.0 / 2.0, 1.0 / 2.0, 0.0 / 2.0]),
    f32x4::new([0.0 / 2.0, 1.0, 0.0 / 2.0, 0.0 / 2.0]),
];

/// Weights for samples `[s0, s1, s2, s3]` at fractional position `t`.
#[inline(always)]
pub fn catmull_rom(t: Sample) -> f32x4 {
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
pub fn catmull_rom_from_powers(g: Sample, gt: Sample, gt2: Sample, gt3: Sample) -> f32x4 {
    let [b0, b1, b2, b3] = CATMULL_ROM;

    let w = b3 * f32x4::splat(g);
    let w = b2.mul_add(f32x4::splat(gt), w);
    let w = b1.mul_add(f32x4::splat(gt2), w);

    b0.mul_add(f32x4::splat(gt3), w)
}
