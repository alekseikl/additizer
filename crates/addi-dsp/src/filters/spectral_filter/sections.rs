use super::simd::ComplexX4;
use crate::{Smp, WideSmp};

pub(super) const TAU: Smp = std::f32::consts::TAU;
pub(super) const TAU_X4: WideSmp = WideSmp::splat(TAU);

#[inline(always)]
fn splat(x: Smp) -> WideSmp {
    WideSmp::splat(x)
}

#[derive(Clone, Copy)]
pub(super) struct LowPass {
    numerator: WideSmp,
    w_squared: WideSmp,
    w_q: WideSmp,
}

impl LowPass {
    pub(super) fn new(gain: Smp, cutoff: Smp, q: Smp) -> Self {
        let w = cutoff * TAU;
        let w_squared = w * w;

        Self {
            numerator: splat(gain * w_squared),
            w_squared: splat(w_squared),
            w_q: splat(w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at_x4(&self, freq: WideSmp) -> ComplexX4 {
        let x = freq * TAU_X4;

        ComplexX4::from_real(self.numerator) / ComplexX4::new(self.w_squared - x * x, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct HighPass {
    neg_gain: WideSmp,
    w_squared: WideSmp,
    w_q: WideSmp,
}

impl HighPass {
    pub(super) fn new(gain: Smp, cutoff: Smp, q: Smp) -> Self {
        let w = cutoff * TAU;

        Self {
            neg_gain: splat(-gain),
            w_squared: splat(w * w),
            w_q: splat(w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at_x4(&self, freq: WideSmp) -> ComplexX4 {
        let x = freq * TAU_X4;
        let x_squared = x * x;

        ComplexX4::from_real(self.neg_gain * x_squared)
            / ComplexX4::new(self.w_squared - x_squared, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct BandPass {
    gain: WideSmp,
    w_squared: WideSmp,
    w_q: WideSmp,
}

impl BandPass {
    pub(super) fn new(gain: Smp, cutoff: Smp, q: Smp) -> Self {
        let w = cutoff * TAU;

        Self {
            gain: splat(gain),
            w_squared: splat(w * w),
            w_q: splat(w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at_x4(&self, freq: WideSmp) -> ComplexX4 {
        let x = freq * TAU_X4;
        let wx_q = self.w_q * x;

        ComplexX4::new(WideSmp::ZERO, self.gain * wx_q)
            / ComplexX4::new(self.w_squared - x * x, wx_q)
    }
}

#[derive(Clone, Copy)]
pub(super) struct Peaking {
    w_squared: WideSmp,
    wa_q: WideSmp,
    w_aq: WideSmp,
}

impl Peaking {
    pub(super) fn new(gain: Smp, cutoff: Smp, q: Smp) -> Self {
        let w = cutoff * TAU;
        let a = gain.max(0.0).sqrt();

        Self {
            w_squared: splat(w * w),
            wa_q: splat((w * a) / q),
            w_aq: splat(w / (a * q)),
        }
    }

    #[inline(always)]
    pub(super) fn at_x4(&self, freq: WideSmp) -> ComplexX4 {
        let x = freq * TAU_X4;
        let wx_diff = self.w_squared - x * x;

        ComplexX4::new(wx_diff, self.wa_q * x) / ComplexX4::new(wx_diff, self.w_aq * x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct Notch {
    gain: WideSmp,
    w_squared: WideSmp,
    w_q: WideSmp,
}

impl Notch {
    pub(super) fn new(gain: Smp, cutoff: Smp, q: Smp) -> Self {
        let w = cutoff * TAU;

        Self {
            gain: splat(gain),
            w_squared: splat(w * w),
            w_q: splat(w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at_x4(&self, freq: WideSmp) -> ComplexX4 {
        let x = freq * TAU_X4;
        let wx_diff = self.w_squared - x * x;

        ComplexX4::from_real(self.gain * wx_diff) / ComplexX4::new(wx_diff, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct OnePoleLowPass {
    w: WideSmp,
}

impl OnePoleLowPass {
    pub(super) fn new(cutoff: Smp) -> Self {
        Self {
            w: splat(cutoff * TAU),
        }
    }

    #[inline(always)]
    pub(super) fn at_x4(&self, freq: WideSmp) -> ComplexX4 {
        let x = freq * TAU_X4;

        ComplexX4::from_real(self.w) / ComplexX4::new(self.w, x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct OnePoleHighPass {
    w: WideSmp,
}

impl OnePoleHighPass {
    pub(super) fn new(cutoff: Smp) -> Self {
        Self {
            w: splat(cutoff * TAU),
        }
    }

    #[inline(always)]
    pub(super) fn at_x4(&self, freq: WideSmp) -> ComplexX4 {
        let x = freq * TAU_X4;

        ComplexX4::new(WideSmp::ZERO, x) / ComplexX4::new(self.w, x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct LowShelf {
    a: WideSmp,
    a_w_sq: WideSmp,
    w_sq: WideSmp,
    w_damp: WideSmp,
}

impl LowShelf {
    pub(super) fn new(gain: Smp, cutoff: Smp, q: Smp) -> Self {
        let w = cutoff * TAU;
        let a = gain.max(0.0).sqrt();
        let w_sq = w * w;

        Self {
            a: splat(a),
            a_w_sq: splat(a * w_sq),
            w_sq: splat(w_sq),
            w_damp: splat(a.sqrt() * w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at_x4(&self, freq: WideSmp) -> ComplexX4 {
        let x = freq * TAU_X4;
        let x_sq = x * x;
        let j_damp = self.w_damp * x;

        (ComplexX4::new(self.a_w_sq - x_sq, j_damp) * self.a)
            / ComplexX4::new(self.w_sq - self.a * x_sq, j_damp)
    }
}

#[derive(Clone, Copy)]
pub(super) struct HighShelf {
    a: WideSmp,
    a_w_sq: WideSmp,
    w_sq: WideSmp,
    w_damp: WideSmp,
}

impl HighShelf {
    pub(super) fn new(gain: Smp, cutoff: Smp, q: Smp) -> Self {
        let w = cutoff * TAU;
        let a = gain.max(0.0).sqrt();
        let w_sq = w * w;

        Self {
            a: splat(a),
            a_w_sq: splat(a * w_sq),
            w_sq: splat(w_sq),
            w_damp: splat(a.sqrt() * w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at_x4(&self, freq: WideSmp) -> ComplexX4 {
        let x = freq * TAU_X4;
        let x_sq = x * x;
        let j_damp = self.w_damp * x;

        (ComplexX4::new(self.w_sq - self.a * x_sq, j_damp) * self.a)
            / ComplexX4::new(self.a_w_sq - x_sq, j_damp)
    }
}
