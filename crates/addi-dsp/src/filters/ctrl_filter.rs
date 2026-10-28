use std::f32::consts::{PI, TAU};

use crate::Smp;

/// `k = 1/Q` at Q = 0.5, the SVF's no-resonance setting.
const SMOOTH_K: Smp = 2.0;
/// Largest `f / sample_rate` fed to `tan`, matching the time-domain SVF.
const MAX_FREQ_RATIO: Smp = 0.499;

/// One Cytomic lowpass used to smooth control signals.
/// Steady state for a constant `value` is `ic2eq = value`.
#[derive(Default, Clone)]
pub struct CtrlFilter {
    ic1eq: Smp,
    ic2eq: Smp,
}

#[derive(Clone, Copy)]
pub struct CtrlFilterCoeffs {
    /// `tan(π f / sample_rate)` for the smoothing cutoff.
    pub g: Smp,
    /// Time constant of the highest cutoff below Nyquist.
    pub min_time: Smp,
}

impl CtrlFilter {
    pub fn reset(&mut self, value: Smp) {
        self.ic1eq = 0.0;
        self.ic2eq = value;
    }

    /// `g = tan(π f / sample_rate)` for a cutoff whose time constant is `time` seconds.
    /// `min_time` is that time constant at the highest cutoff below Nyquist.
    pub fn coeffs(time: Smp, sample_rate: Smp) -> CtrlFilterCoeffs {
        let min_time = (TAU * MAX_FREQ_RATIO * sample_rate).recip();
        let fc = (TAU * time.max(min_time)).recip();
        let ratio = (fc / sample_rate).min(MAX_FREQ_RATIO);

        CtrlFilterCoeffs {
            g: (PI * ratio).tan(),
            min_time,
        }
    }

    #[inline(always)]
    pub fn tick(&mut self, g: Smp, input: Smp) -> Smp {
        let a1 = 1.0 / g.mul_add(g + SMOOTH_K, 1.0);
        let v3 = input - self.ic2eq;
        let v1 = a1 * g.mul_add(v3, self.ic1eq);
        let v2 = g.mul_add(v1, self.ic2eq);

        self.ic1eq = v1.mul_add(2.0, -self.ic1eq);
        self.ic2eq = v2.mul_add(2.0, -self.ic2eq);
        v2
    }
}

#[cfg(test)]
mod tests;
