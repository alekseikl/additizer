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
