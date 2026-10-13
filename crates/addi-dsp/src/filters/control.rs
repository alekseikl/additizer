use wide::f32x4;

use crate::Sample;

pub const MIN_DRIVE: Sample = -60.0;
pub const MAX_DRIVE: Sample = 24.0;
pub const MIN_RESONANCE: Sample = -1.0;
pub const MAX_RESONANCE: Sample = 1.0;
/// Q of the resonant stage when the resonance control is 0.
pub const ZERO_RESONANCE_Q: Sample = 0.5;
/// Pre-stage Q at and below zero resonance.
pub const MIN_PRE_Q: Sample = 0.5;
/// Pre-stage Q at full resonance. The spectral EQ always uses this.
pub const MAX_PRE_Q: Sample = 1.0;
/// Q of the resonant stage when the resonance control is fully negative.
pub const NEGATIVE_RESONANCE_Q: Sample = 0.01;
pub const MAX_RESONANCE_Q: Sample = 20.0;

/// Q of the pre stage and of the resonant stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResonanceQ {
    pub pre_stage: Sample,
    pub resonant: Sample,
}

/// Maps a resonance control to both stage Qs.
///
/// Positive resonance cubes the resonant stage from [`ZERO_RESONANCE_Q`] to [`MAX_RESONANCE_Q`]
/// and moves the pre-stage Q with the square root of resonance from [`MIN_PRE_Q`] to
/// [`MAX_PRE_Q`]. At and below zero the pre-stage Q stays at [`MIN_PRE_Q`], and the
/// resonant stage falls linearly to [`NEGATIVE_RESONANCE_Q`].
#[inline(always)]
pub fn q_from_resonance(resonance: Sample) -> ResonanceQ {
    let r = resonance.clamp(MIN_RESONANCE, MAX_RESONANCE);

    if r > 0.0 {
        let curve = r * r * r;

        ResonanceQ {
            pre_stage: MIN_PRE_Q + (MAX_PRE_Q - MIN_PRE_Q) * r.sqrt(),
            resonant: ZERO_RESONANCE_Q + (MAX_RESONANCE_Q - ZERO_RESONANCE_Q) * curve,
        }
    } else {
        ResonanceQ {
            pre_stage: MIN_PRE_Q,
            resonant: NEGATIVE_RESONANCE_Q + (ZERO_RESONANCE_Q - NEGATIVE_RESONANCE_Q) * (1.0 + r),
        }
    }
}

/// [`q_from_resonance`] for four lanes. Returns `(resonant Q, pre-stage Q)`.
#[inline(always)]
pub fn q_from_resonance_x4(resonance: f32x4) -> (f32x4, f32x4) {
    const MIN_R: f32x4 = f32x4::splat(MIN_RESONANCE);
    const MAX_R: f32x4 = f32x4::splat(MAX_RESONANCE);
    const ZERO_Q: f32x4 = f32x4::splat(ZERO_RESONANCE_Q);
    const MAX_Q: f32x4 = f32x4::splat(MAX_RESONANCE_Q);
    const NEG_Q: f32x4 = f32x4::splat(NEGATIVE_RESONANCE_Q);
    const MIN_PRE: f32x4 = f32x4::splat(MIN_PRE_Q);
    const MAX_PRE: f32x4 = f32x4::splat(MAX_PRE_Q);
    const ONE: f32x4 = f32x4::ONE;

    let r = resonance.max(MIN_R).min(MAX_R);
    let positive = r.simd_gt(f32x4::ZERO);
    // Negative lanes would make `sqrt` NaN. Those lanes take the linear branch below.
    let curve = r * r * r;
    let pre_pos = MIN_PRE + (MAX_PRE - MIN_PRE) * positive.select(r, f32x4::ZERO).sqrt();
    let res_pos = ZERO_Q + (MAX_Q - ZERO_Q) * curve;
    let res_neg = NEG_Q + (ZERO_Q - NEG_Q) * (ONE + r);

    (
        positive.select(res_pos, res_neg),
        positive.select(pre_pos, MIN_PRE),
    )
}
