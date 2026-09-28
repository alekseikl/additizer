use crate::synth_engine::Sample;

pub const MIN_DRIVE: Sample = -60.0;
pub const MAX_DRIVE: Sample = 24.0;
pub const MIN_RESONANCE: Sample = -1.0;
pub const MAX_RESONANCE: Sample = 1.0;
/// Q of the resonant stage when the resonance control is 0.
pub const ZERO_RESONANCE_Q: Sample = 0.5;
pub const MIN_RESONANCE_Q: Sample = 0.01;
pub const MAX_RESONANCE_Q: Sample = 16.0;

#[inline(always)]
pub fn q_from_resonance(resonance: Sample) -> Sample {
    let r = resonance.clamp(MIN_RESONANCE, MAX_RESONANCE);

    if r > 0.0 {
        ZERO_RESONANCE_Q + (MAX_RESONANCE_Q - ZERO_RESONANCE_Q) * r * r * r
    } else {
        let r = 1.0 + r;
        MIN_RESONANCE_Q + (ZERO_RESONANCE_Q - MIN_RESONANCE_Q) * r * r * r
    }
}
