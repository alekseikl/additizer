use crate::{ComplexSample, Sample};

pub(super) const TAU: Sample = std::f32::consts::TAU;

#[derive(Clone, Copy)]
pub(super) struct LowPass {
    numerator: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl LowPass {
    pub(super) fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;
        let w_squared = w * w;

        Self {
            numerator: gain * w_squared,
            w_squared,
            w_q: w / q,
        }
    }

    #[inline]
    pub(super) fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;

        self.numerator / ComplexSample::new(self.w_squared - x * x, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct HighPass {
    neg_gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl HighPass {
    pub(super) fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            neg_gain: -gain,
            w_squared: w * w,
            w_q: w / q,
        }
    }

    #[inline]
    pub(super) fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_squared = x * x;

        (self.neg_gain * x_squared) / ComplexSample::new(self.w_squared - x_squared, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct BandPass {
    gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl BandPass {
    pub(super) fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            gain,
            w_squared: w * w,
            w_q: w / q,
        }
    }

    #[inline]
    pub(super) fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let wx_q = self.w_q * x;

        ComplexSample::new(0.0, self.gain * wx_q) / ComplexSample::new(self.w_squared - x * x, wx_q)
    }
}

#[derive(Clone, Copy)]
pub(super) struct Peaking {
    w_squared: Sample,
    wa_q: Sample,
    w_aq: Sample,
}

impl Peaking {
    pub(super) fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;
        let a = gain.max(0.0).sqrt();

        Self {
            w_squared: w * w,
            wa_q: (w * a) / q,
            w_aq: w / (a * q),
        }
    }

    #[inline]
    pub(super) fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let wx_diff = self.w_squared - x * x;

        ComplexSample::new(wx_diff, self.wa_q * x) / ComplexSample::new(wx_diff, self.w_aq * x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct Notch {
    gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl Notch {
    pub(super) fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            gain,
            w_squared: w * w,
            w_q: w / q,
        }
    }

    #[inline]
    pub(super) fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let wx_diff = self.w_squared - x * x;

        (self.gain * wx_diff) / ComplexSample::new(wx_diff, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct OnePoleLowPass {
    w: Sample,
}

impl OnePoleLowPass {
    pub(super) fn new(cutoff: Sample) -> Self {
        Self { w: cutoff * TAU }
    }

    #[inline]
    pub(super) fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;

        self.w / ComplexSample::new(self.w, x)
    }
}

#[derive(Clone, Copy)]
pub(super) struct OnePoleHighPass {
    w: Sample,
}

impl OnePoleHighPass {
    pub(super) fn new(cutoff: Sample) -> Self {
        Self { w: cutoff * TAU }
    }

    #[inline]
    pub(super) fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;

        ComplexSample::new(0.0, x) / ComplexSample::new(self.w, x)
    }
}

/// `A = √gain` and the shared shelf terms for one section.
#[derive(Clone, Copy)]
struct ShelfCoeffs {
    a: Sample,
    a_w_sq: Sample,
    w_sq: Sample,
    w_damp: Sample,
}

impl ShelfCoeffs {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;
        let a = gain.max(0.0).sqrt();
        let w_sq = w * w;

        Self {
            a,
            a_w_sq: a * w_sq,
            w_sq,
            w_damp: a.sqrt() * w / q,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct LowShelf {
    coeffs: ShelfCoeffs,
}

impl LowShelf {
    pub(super) fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            coeffs: ShelfCoeffs::new(gain, cutoff, q),
        }
    }

    #[inline]
    pub(super) fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_sq = x * x;
        let j_damp = self.coeffs.w_damp * x;
        let c = &self.coeffs;

        c.a * ComplexSample::new(c.a_w_sq - x_sq, j_damp)
            / ComplexSample::new(c.w_sq - c.a * x_sq, j_damp)
    }
}

#[derive(Clone, Copy)]
pub(super) struct HighShelf {
    coeffs: ShelfCoeffs,
}

impl HighShelf {
    pub(super) fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            coeffs: ShelfCoeffs::new(gain, cutoff, q),
        }
    }

    #[inline]
    pub(super) fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_sq = x * x;
        let j_damp = self.coeffs.w_damp * x;
        let c = &self.coeffs;

        c.a * ComplexSample::new(c.w_sq - c.a * x_sq, j_damp)
            / ComplexSample::new(c.a_w_sq - x_sq, j_damp)
    }
}
