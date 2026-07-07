use std::f32;

use nih_plug::util::db_to_gain_fast;

use crate::synth_engine::{ComplexSample, Sample};

const TAU: Sample = f32::consts::TAU;

pub trait FilterImpl: Clone + Copy + 'static {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self;
    fn at(&self, freq: Sample) -> ComplexSample;
}

#[derive(Clone, Copy)]
pub struct LowPass12 {
    numerator: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl FilterImpl for LowPass12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;
        let w_squared = w * w;

        Self {
            numerator: gain * w_squared,
            w_squared,
            w_q: w / q,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;

        self.numerator / ComplexSample::new(self.w_squared - x * x, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub struct LowPass18 {
    numerator: Sample,
    w_squared: Sample,
    w: Sample,
    w_q: Sample,
}

impl FilterImpl for LowPass18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;
        let w_squared = w * w;

        Self {
            numerator: gain * w_squared * w,
            w_squared,
            w,
            w_q: w / q,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_squared = x * x;
        let wx_diff = self.w_squared - x_squared;

        self.numerator
            / ComplexSample::new(
                self.w * wx_diff - self.w_q * x_squared,
                x * (self.w + self.w_q * wx_diff),
            )
    }
}

const BUTTERWORTH_Q: Sample = f32::consts::FRAC_1_SQRT_2;

#[derive(Clone, Copy)]
pub struct LowPass24 {
    numerator: Sample,
    w_squared: Sample,
    w_q1: Sample,
    w_q2: Sample,
}

impl FilterImpl for LowPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;
        let w_squared = w * w;

        Self {
            numerator: gain * w_squared * w_squared,
            w_squared,
            w_q1: w / BUTTERWORTH_Q,
            w_q2: w / q,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_squared = x * x;
        let wx_diff = self.w_squared - x_squared;

        self.numerator
            / ComplexSample::new(
                wx_diff * wx_diff - self.w_q1 * self.w_q2 * x_squared,
                wx_diff * x * (self.w_q1 + self.w_q2),
            )
    }
}

#[derive(Clone, Copy)]
pub struct HighPass12 {
    neg_gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl FilterImpl for HighPass12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            neg_gain: -gain,
            w_squared: w * w,
            w_q: w / q,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_squared = x * x;

        (self.neg_gain * x_squared) / ComplexSample::new(self.w_squared - x_squared, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub struct HighPass18 {
    neg_gain: Sample,
    w_squared: Sample,
    w: Sample,
    w_q: Sample,
}

impl FilterImpl for HighPass18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            neg_gain: -gain,
            w_squared: w * w,
            w,
            w_q: w / q,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_squared = x * x;
        let wx_diff = self.w_squared - x_squared;

        ComplexSample::new(0.0, self.neg_gain * x_squared * x)
            / ComplexSample::new(
                self.w * wx_diff - self.w_q * x_squared,
                x * (self.w + self.w_q * wx_diff),
            )
    }
}

#[derive(Clone, Copy)]
pub struct HighPass24 {
    neg_gain: Sample,
    w_squared: Sample,
    w_q1: Sample,
    w_q2: Sample,
}

impl FilterImpl for HighPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            neg_gain: -gain,
            w_squared: w * w,
            w_q1: w / BUTTERWORTH_Q,
            w_q2: w / q,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_squared = x * x;
        let x_fourth = x_squared * x_squared;
        let wx_diff = self.w_squared - x_squared;

        (-self.neg_gain * x_fourth)
            / ComplexSample::new(
                wx_diff * wx_diff - self.w_q1 * self.w_q2 * x_squared,
                wx_diff * x * (self.w_q1 + self.w_q2),
            )
    }
}

#[derive(Clone, Copy)]
pub struct BandPass {
    gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl FilterImpl for BandPass {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            gain,
            w_squared: w * w,
            w_q: w / q,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let wx_q = self.w_q * x;

        ComplexSample::new(0.0, self.gain * wx_q) / ComplexSample::new(self.w_squared - x * x, wx_q)
    }
}

#[derive(Clone, Copy)]
pub struct Peaking {
    w_squared: Sample,
    wa_q: Sample,
    w_aq: Sample,
}

impl FilterImpl for Peaking {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            w_squared: w * w,
            wa_q: (w * gain) / q,
            w_aq: w / (gain * q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let wx_diff = self.w_squared - x * x;

        ComplexSample::new(wx_diff, self.wa_q * x) / ComplexSample::new(wx_diff, self.w_aq * x)
    }
}

#[derive(Clone, Copy)]
pub struct Notch {
    gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl FilterImpl for Notch {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            gain,
            w_squared: w * w,
            w_q: w / q,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let wx_diff = self.w_squared - x * x;

        (self.gain * wx_diff) / ComplexSample::new(wx_diff, self.w_q * x)
    }
}

pub enum FilterType {
    LowPass12,
    LowPass18,
    LowPass24,
    HighPass12,
    HighPass18,
    HighPass24,
    BandPass,
    Peaking,
    Notch,
}

pub struct FilterParams {
    pub drive: Sample,
    pub cutoff: Sample,
    pub resonance: Sample,
    pub linear_phase: bool,
}

pub const MAX_DRIVE: Sample = 40.0;
pub const MIN_RESONANCE: Sample = -1.0;
pub const MAX_RESONANCE: Sample = 1.0;
pub const MIN_CUTOFF: Sample = -2.0;
pub const MAX_CUTOFF: Sample = 10.0;
const MIN_Q: Sample = 0.01;
const MAX_Q: Sample = 16.0;

pub struct SpectralFilter {
    filter_type: FilterType,
    gain: Sample,
    cutoff: Sample,
    q: Sample,
    linear_phase: bool,
}

impl SpectralFilter {
    pub fn new(filter_type: FilterType, params: FilterParams) -> Self {
        let resonance = params.resonance.clamp(MIN_RESONANCE, MAX_RESONANCE);

        Self {
            filter_type,
            gain: db_to_gain_fast(params.drive.max(MAX_DRIVE)),
            cutoff: params.cutoff.clamp(MIN_CUTOFF, MAX_CUTOFF),
            q: if resonance > 0.0 {
                BUTTERWORTH_Q + (MAX_Q - BUTTERWORTH_Q) * resonance
            } else {
                MIN_Q + (BUTTERWORTH_Q - MIN_Q) * -resonance
            },
            linear_phase: params.linear_phase,
        }
    }

    pub fn apply_response(&self, input: &[ComplexSample], output: &mut [ComplexSample]) {
        match self.filter_type {
            FilterType::LowPass12 => self.apply_impl::<LowPass12>(input, output),
            FilterType::LowPass18 => self.apply_impl::<LowPass18>(input, output),
            FilterType::LowPass24 => self.apply_impl::<LowPass24>(input, output),
            FilterType::HighPass12 => self.apply_impl::<HighPass12>(input, output),
            FilterType::HighPass18 => self.apply_impl::<HighPass18>(input, output),
            FilterType::HighPass24 => self.apply_impl::<HighPass24>(input, output),
            FilterType::BandPass => self.apply_impl::<BandPass>(input, output),
            FilterType::Peaking => self.apply_impl::<Peaking>(input, output),
            FilterType::Notch => self.apply_impl::<Notch>(input, output),
        }
    }

    fn apply_impl<T: FilterImpl>(&self, input: &[ComplexSample], output: &mut [ComplexSample]) {
        let filter_impl = T::new(self.gain, self.cutoff, self.q);

        if self.linear_phase {
            for (i, (out, &inp)) in output.iter_mut().zip(input).enumerate() {
                *out = inp * filter_impl.at(i as Sample).norm();
            }
        } else {
            for (i, (out, &inp)) in output.iter_mut().zip(input).enumerate() {
                *out = inp * filter_impl.at(i as Sample);
            }
        }
    }
}
