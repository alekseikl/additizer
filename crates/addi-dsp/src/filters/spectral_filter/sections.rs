use super::simd::WideComplex;
use crate::{Smp, WideSmp};

pub(super) const TAU: Smp = std::f32::consts::TAU;
pub(super) const TAU_WIDE: WideSmp = WideSmp::splat(TAU);

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
            numerator: WideSmp::splat(gain * w_squared),
            w_squared: WideSmp::splat(w_squared),
            w_q: WideSmp::splat(w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at(&self, freq: WideSmp) -> WideComplex {
        let x = freq * TAU_WIDE;

        WideComplex::from_real(self.numerator)
            / WideComplex::new(self.w_squared - x * x, self.w_q * x)
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
            neg_gain: WideSmp::splat(-gain),
            w_squared: WideSmp::splat(w * w),
            w_q: WideSmp::splat(w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at(&self, freq: WideSmp) -> WideComplex {
        let x = freq * TAU_WIDE;
        let x_squared = x * x;

        WideComplex::from_real(self.neg_gain * x_squared)
            / WideComplex::new(self.w_squared - x_squared, self.w_q * x)
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
            gain: WideSmp::splat(gain),
            w_squared: WideSmp::splat(w * w),
            w_q: WideSmp::splat(w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at(&self, freq: WideSmp) -> WideComplex {
        let x = freq * TAU_WIDE;
        let wx_q = self.w_q * x;

        WideComplex::new(WideSmp::ZERO, self.gain * wx_q)
            / WideComplex::new(self.w_squared - x * x, wx_q)
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
            w_squared: WideSmp::splat(w * w),
            wa_q: WideSmp::splat((w * a) / q),
            w_aq: WideSmp::splat(w / (a * q)),
        }
    }

    #[inline(always)]
    pub(super) fn at(&self, freq: WideSmp) -> WideComplex {
        let x = freq * TAU_WIDE;
        let wx_diff = self.w_squared - x * x;

        WideComplex::new(wx_diff, self.wa_q * x) / WideComplex::new(wx_diff, self.w_aq * x)
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
            gain: WideSmp::splat(gain),
            w_squared: WideSmp::splat(w * w),
            w_q: WideSmp::splat(w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at(&self, freq: WideSmp) -> WideComplex {
        let x = freq * TAU_WIDE;
        let wx_diff = self.w_squared - x * x;

        WideComplex::from_real(self.gain * wx_diff) / WideComplex::new(wx_diff, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct OnePoleLowPass {
    w: WideSmp,
}

impl OnePoleLowPass {
    pub(super) fn new(cutoff: Smp) -> Self {
        Self {
            w: WideSmp::splat(cutoff * TAU),
        }
    }

    #[inline(always)]
    pub(super) fn at(&self, freq: WideSmp) -> WideComplex {
        let x = freq * TAU_WIDE;

        WideComplex::from_real(self.w) / WideComplex::new(self.w, x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct OnePoleHighPass {
    w: WideSmp,
}

impl OnePoleHighPass {
    pub(super) fn new(cutoff: Smp) -> Self {
        Self {
            w: WideSmp::splat(cutoff * TAU),
        }
    }

    #[inline(always)]
    pub(super) fn at(&self, freq: WideSmp) -> WideComplex {
        let x = freq * TAU_WIDE;

        WideComplex::new(WideSmp::ZERO, x) / WideComplex::new(self.w, x)
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
            a: WideSmp::splat(a),
            a_w_sq: WideSmp::splat(a * w_sq),
            w_sq: WideSmp::splat(w_sq),
            w_damp: WideSmp::splat(a.sqrt() * w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at(&self, freq: WideSmp) -> WideComplex {
        let x = freq * TAU_WIDE;
        let x_sq = x * x;
        let j_damp = self.w_damp * x;

        (WideComplex::new(self.a_w_sq - x_sq, j_damp) * self.a)
            / WideComplex::new(self.w_sq - self.a * x_sq, j_damp)
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
            a: WideSmp::splat(a),
            a_w_sq: WideSmp::splat(a * w_sq),
            w_sq: WideSmp::splat(w_sq),
            w_damp: WideSmp::splat(a.sqrt() * w / q),
        }
    }

    #[inline(always)]
    pub(super) fn at(&self, freq: WideSmp) -> WideComplex {
        let x = freq * TAU_WIDE;
        let x_sq = x * x;
        let j_damp = self.w_damp * x;

        (WideComplex::new(self.w_sq - self.a * x_sq, j_damp) * self.a)
            / WideComplex::new(self.a_w_sq - x_sq, j_damp)
    }
}
