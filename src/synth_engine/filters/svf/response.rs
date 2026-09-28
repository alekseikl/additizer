use super::SvfType;
use crate::synth_engine::{ComplexSample, Sample};

/// Analog prototype frequency response. Frequencies passed to [`Self::at`] use the same units as `cutoff`.
///
/// `gain` is the linear level of a [`SvfType::Peaking`] bell at its cutoff, the DC level of a low shelf,
/// and the high-frequency level of a high shelf.
#[derive(Clone, Copy)]
pub struct SvfResponse {
    pub filter_type: SvfType,
    pub q: Sample,
    pub cutoff: Sample,
    pub gain: Sample,
}

impl SvfResponse {
    #[inline(always)]
    fn low_pass(k: Sample, s: ComplexSample) -> ComplexSample {
        (s * s + s * k + 1.0).inv()
    }

    #[inline(always)]
    fn one_pole_low_pass(s: ComplexSample) -> ComplexSample {
        (s + 1.0).inv()
    }

    #[inline(always)]
    fn high_pass(k: Sample, s: ComplexSample) -> ComplexSample {
        let d = s * s + s * k + 1.0;

        s * s / d
    }

    #[inline(always)]
    fn one_pole_high_pass(s: ComplexSample) -> ComplexSample {
        s / (s + 1.0)
    }

    #[inline(always)]
    fn band_pass(k: Sample, s: ComplexSample) -> ComplexSample {
        let d = s * s + s * k + 1.0;

        s * k / d
    }

    fn notch(q: Sample, s: ComplexSample) -> ComplexSample {
        let k = q.recip();
        let d = s * s + s * k + 1.0;

        (s * s + 1.0) / d
    }

    /// `(s^2 + (A/Q) s + 1) / (s^2 + s/(A*Q) + 1)`, with `A^2 = gain`.
    fn peaking(q: Sample, gain: Sample, s: ComplexSample) -> ComplexSample {
        let gain = gain.max(1e-4);
        let k = q.recip() / gain.sqrt();
        let s2 = s * s;
        let d = s2 + s * k + 1.0;

        (s2 + s * gain * k + 1.0) / d
    }

    /// `A * (s^2 + (√A/Q) s + A) / (A s^2 + (√A/Q) s + 1)`, with `A^2 = gain`.
    fn low_shelf(q: Sample, gain: Sample, s: ComplexSample) -> ComplexSample {
        let gain = gain.max(1e-4);
        let a = gain.sqrt();
        let s2 = s * s;
        let damp = s * (a.sqrt() / q);

        a * (s2 + damp + a) / (s2 * a + damp + 1.0)
    }

    /// `A * (A s^2 + (√A/Q) s + 1) / (s^2 + (√A/Q) s + A)`, with `A^2 = gain`.
    fn high_shelf(q: Sample, gain: Sample, s: ComplexSample) -> ComplexSample {
        let gain = gain.max(1e-4);
        let a = gain.sqrt();
        let s2 = s * s;
        let damp = s * (a.sqrt() / q);

        a * (s2 * a + damp + 1.0) / (s2 + damp + a)
    }

    fn of_type(filter_type: SvfType, q: Sample, gain: Sample, s: ComplexSample) -> ComplexSample {
        match filter_type {
            SvfType::LowPass12 => Self::low_pass(q.recip(), s),
            SvfType::LowPass18 => Self::low_pass(q.recip(), s) * Self::one_pole_low_pass(s),
            SvfType::LowPass24 => Self::low_pass(q.recip(), s) * Self::low_pass(1.0, s),
            SvfType::HighPass12 => Self::high_pass(q.recip(), s),
            SvfType::HighPass18 => Self::high_pass(q.recip(), s) * Self::one_pole_high_pass(s),
            SvfType::HighPass24 => Self::high_pass(q.recip(), s) * Self::high_pass(1.0, s),
            SvfType::BandPass6 => Self::band_pass(q.recip(), s),
            SvfType::BandPass12 => Self::band_pass(q.recip(), s) * Self::band_pass(1.0, s),
            SvfType::Peaking => Self::peaking(q, gain, s),
            SvfType::Notch => Self::notch(q, s),
            SvfType::LowShelf12 => Self::low_shelf(q, gain, s),
            SvfType::HighShelf12 => Self::high_shelf(q, gain, s),
        }
    }

    pub fn at(&self, freq: Sample) -> ComplexSample {
        let s = ComplexSample::new(0.0, freq / self.cutoff);

        Self::of_type(self.filter_type, self.q, self.gain, s)
    }
}
