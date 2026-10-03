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
