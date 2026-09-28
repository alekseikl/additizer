use super::{BandPass, HighPass, LowPass, SvfType};
use crate::synth_engine::{ComplexSample, Sample};

/// Analog prototype of an SVF output tap.
trait Response {
    fn response(k: Sample, s: ComplexSample) -> ComplexSample;
}

/// Analog prototype of a one-pole section cascaded after an SVF stage.
trait OnePoleResponse {
    fn one_pole_response(s: ComplexSample) -> ComplexSample;
}

impl Response for LowPass {
    #[inline(always)]
    fn response(k: Sample, s: ComplexSample) -> ComplexSample {
        (s * s + s * k + 1.0).inv()
    }
}

impl OnePoleResponse for LowPass {
    #[inline(always)]
    fn one_pole_response(s: ComplexSample) -> ComplexSample {
        (s + 1.0).inv()
    }
}

impl Response for HighPass {
    #[inline(always)]
    fn response(k: Sample, s: ComplexSample) -> ComplexSample {
        let d = s * s + s * k + 1.0;

        s * s / d
    }
}

impl OnePoleResponse for HighPass {
    #[inline(always)]
    fn one_pole_response(s: ComplexSample) -> ComplexSample {
        s / (s + 1.0)
    }
}

impl Response for BandPass {
    #[inline(always)]
    fn response(k: Sample, s: ComplexSample) -> ComplexSample {
        let d = s * s + s * k + 1.0;

        s * k / d
    }
}

/// Analog prototype frequency response. Frequencies passed to [`Self::at`] use the same units as `cutoff`.
///
/// `gain` is the linear level of a [`SvfType::Peaking`] bell at its cutoff.
#[derive(Clone, Copy)]
pub struct SvfResponse {
    pub filter_type: SvfType,
    pub q: Sample,
    pub cutoff: Sample,
    pub gain: Sample,
}

impl SvfResponse {
    fn response_2<S: Response>(q: Sample, s: ComplexSample) -> ComplexSample {
        S::response(q.recip(), s)
    }

    fn response_3<S: Response + OnePoleResponse>(q: Sample, s: ComplexSample) -> ComplexSample {
        S::response(q.recip(), s) * S::one_pole_response(s)
    }

    fn response_4<S: Response>(q: Sample, s: ComplexSample) -> ComplexSample {
        S::response(q.recip(), s) * S::response(1.0, s)
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

    fn of_type(filter_type: SvfType, q: Sample, gain: Sample, s: ComplexSample) -> ComplexSample {
        match filter_type {
            SvfType::LowPass12 => Self::response_2::<LowPass>(q, s),
            SvfType::LowPass18 => Self::response_3::<LowPass>(q, s),
            SvfType::LowPass24 => Self::response_4::<LowPass>(q, s),
            SvfType::HighPass12 => Self::response_2::<HighPass>(q, s),
            SvfType::HighPass18 => Self::response_3::<HighPass>(q, s),
            SvfType::HighPass24 => Self::response_4::<HighPass>(q, s),
            SvfType::BandPass6 => Self::response_2::<BandPass>(q, s),
            SvfType::BandPass12 => Self::response_4::<BandPass>(q, s),
            SvfType::Peaking => Self::peaking(q, gain, s),
            SvfType::Notch => Self::notch(q, s),
        }
    }

    pub fn at(&self, freq: Sample) -> ComplexSample {
        let s = ComplexSample::new(0.0, freq / self.cutoff);

        Self::of_type(self.filter_type, self.q, self.gain, s)
    }
}
