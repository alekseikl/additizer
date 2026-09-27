use super::{BandPass, HighPass, LowPass, SvfType, q_from_resonance};
use crate::synth_engine::{ComplexSample, Sample};

/// Analog prototype of an SVF output tap.
trait Response {
    fn response(k: Sample, s: ComplexSample) -> ComplexSample;
    fn one_pole_response(s: ComplexSample) -> ComplexSample;
}

impl Response for LowPass {
    #[inline(always)]
    fn response(k: Sample, s: ComplexSample) -> ComplexSample {
        (s * s + s * k + 1.0).inv()
    }

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

    #[inline(always)]
    fn one_pole_response(s: ComplexSample) -> ComplexSample {
        (s + 1.0).inv()
    }
}

/// Analog prototype frequency response. Frequencies passed to [`Self::at`] use the same units as `cutoff`.
#[derive(Clone, Copy)]
pub struct SvfResponse {
    pub filter_type: SvfType,
    pub resonance: Sample,
    pub cutoff: Sample,
}

impl SvfResponse {
    fn response_2<S: Response>(q: Sample, s: ComplexSample) -> ComplexSample {
        S::response(q.recip(), s)
    }

    fn response_3<S: Response>(q: Sample, s: ComplexSample) -> ComplexSample {
        S::response(q.recip(), s) * S::one_pole_response(s)
    }

    fn response_4<S: Response>(q: Sample, s: ComplexSample) -> ComplexSample {
        S::response(q.recip(), s) * S::response(1.0, s)
    }

    fn of_type(filter_type: SvfType, q: Sample, s: ComplexSample) -> ComplexSample {
        match filter_type {
            SvfType::LowPass12 => Self::response_2::<LowPass>(q, s),
            SvfType::LowPass18 => Self::response_3::<LowPass>(q, s),
            SvfType::LowPass24 => Self::response_4::<LowPass>(q, s),
            SvfType::HighPass12 => Self::response_2::<HighPass>(q, s),
            SvfType::HighPass18 => Self::response_3::<HighPass>(q, s),
            SvfType::HighPass24 => Self::response_4::<HighPass>(q, s),
            SvfType::BandPass6 => Self::response_2::<BandPass>(q, s),
            SvfType::BandPass12 => Self::response_4::<BandPass>(q, s),
        }
    }

    pub fn at(&self, freq: Sample) -> ComplexSample {
        let q = q_from_resonance(self.resonance);
        let s = ComplexSample::new(0.0, freq / self.cutoff);

        Self::of_type(self.filter_type, q, s)
    }
}
