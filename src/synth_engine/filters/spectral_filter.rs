use std::f32;

use nih_plug::util::db_to_gain_fast;
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ComplexSample, Sample},
    utils::{power_scale, st_to_octave},
};

const TAU: Sample = f32::consts::TAU;

pub trait FilterImpl: Clone + Copy + 'static {
    fn new(gain: Sample, cutoff_freq: Sample, q: Sample) -> Self;
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
    biquad: LowPass12,
    w: Sample,
}

impl FilterImpl for LowPass18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            biquad: LowPass12::new(gain, cutoff, q),
            w: cutoff * TAU,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let one_pole = self.w / ComplexSample::new(self.w, x);

        one_pole * self.biquad.at(freq)
    }
}

const BUTTERWORTH_Q: Sample = f32::consts::FRAC_1_SQRT_2;

#[derive(Clone, Copy)]
pub struct LowPass24 {
    butterworth_section: LowPass12,
    user_section: LowPass12,
}

impl FilterImpl for LowPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            butterworth_section: LowPass12::new(1.0, cutoff, BUTTERWORTH_Q),
            user_section: LowPass12::new(gain, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        self.butterworth_section.at(freq) * self.user_section.at(freq)
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
    biquad: HighPass12,
    w: Sample,
}

impl FilterImpl for HighPass18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            biquad: HighPass12::new(gain, cutoff, q),
            w: cutoff * TAU,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let one_pole = ComplexSample::new(0.0, x) / ComplexSample::new(self.w, x);

        one_pole * self.biquad.at(freq)
    }
}

#[derive(Clone, Copy)]
pub struct HighPass24 {
    butterworth_section: HighPass12,
    user_section: HighPass12,
}

impl FilterImpl for HighPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            butterworth_section: HighPass12::new(1.0, cutoff, BUTTERWORTH_Q),
            user_section: HighPass12::new(gain, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        self.butterworth_section.at(freq) * self.user_section.at(freq)
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

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FilterType {
    #[default]
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

impl FilterType {
    pub const ALL: [Self; 9] = [
        Self::LowPass12,
        Self::LowPass18,
        Self::LowPass24,
        Self::HighPass12,
        Self::HighPass18,
        Self::HighPass24,
        Self::BandPass,
        Self::Peaking,
        Self::Notch,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::LowPass12 => "Lowpass 12",
            Self::LowPass18 => "Lowpass 18",
            Self::LowPass24 => "Lowpass 24",
            Self::HighPass12 => "Highpass 12",
            Self::HighPass18 => "Highpass 18",
            Self::HighPass24 => "Highpass 24",
            Self::BandPass => "Bandpass",
            Self::Peaking => "Peaking",
            Self::Notch => "Notch",
        }
    }
}

pub struct FilterParams {
    pub drive: Sample,
    pub cutoff: Sample, // Octaves
    pub resonance: Sample,
    pub q_limit_to: Sample,    // Octaves. Before this point Q is limited.
    pub q_limit_curve: Sample, // [0.0-1.0]
    pub linear_phase: bool,
}

pub const MAX_DRIVE: Sample = 40.0;
pub const MIN_RESONANCE: Sample = -1.0;
pub const MAX_RESONANCE: Sample = 1.0;
pub const MIN_CUTOFF: Sample = -4.0;
pub const MAX_CUTOFF: Sample = 10.0;
const MIN_Q: Sample = 0.01;
const MAX_Q: Sample = 16.0;
const MIN_Q_LIMIT: Sample = 0.0;
const MAX_Q_LIMIT: Sample = 10.0;
const MAX_Q_LIMIT_POWER: Sample = 10.0;

pub struct SpectralFilter {
    filter_type: FilterType,
    gain: Sample,
    cutoff_freq: Sample,
    q: Sample,
    linear_phase: bool,
}

impl SpectralFilter {
    pub fn new(filter_type: FilterType, params: FilterParams) -> Self {
        Self {
            filter_type,
            gain: db_to_gain_fast(params.drive.min(MAX_DRIVE)),
            cutoff_freq: params.cutoff.clamp(MIN_CUTOFF, MAX_CUTOFF).exp2(),
            q: Self::q_from_params(&params),
            linear_phase: params.linear_phase,
        }
    }

    fn q_from_params(params: &FilterParams) -> Sample {
        let resonance = params.resonance.clamp(MIN_RESONANCE, MAX_RESONANCE);
        let q_limit_to = params.q_limit_to.clamp(MIN_Q_LIMIT, MAX_Q_LIMIT);

        let q = if resonance > 0.0 {
            BUTTERWORTH_Q + (MAX_Q - BUTTERWORTH_Q) * resonance.powf(3.0)
        } else {
            MIN_Q + (BUTTERWORTH_Q - MIN_Q) * (1.0 + resonance)
        };

        let butterworth_excess = q - BUTTERWORTH_Q;

        if q_limit_to < st_to_octave(1.0) || butterworth_excess <= 0.0 || params.cutoff > q_limit_to
        {
            return q;
        }

        let q_limit_curve = params.q_limit_curve.clamp(0.0, 1.0) * MAX_Q_LIMIT_POWER;
        let t = params.cutoff.max(0.0) / q_limit_to;

        BUTTERWORTH_Q + butterworth_excess * power_scale(t, q_limit_curve)
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

    pub fn response_at(&self, freq: Sample) -> ComplexSample {
        match self.filter_type {
            FilterType::LowPass12 => self.response_impl::<LowPass12>(freq),
            FilterType::LowPass18 => self.response_impl::<LowPass18>(freq),
            FilterType::LowPass24 => self.response_impl::<LowPass24>(freq),
            FilterType::HighPass12 => self.response_impl::<HighPass12>(freq),
            FilterType::HighPass18 => self.response_impl::<HighPass18>(freq),
            FilterType::HighPass24 => self.response_impl::<HighPass24>(freq),
            FilterType::BandPass => self.response_impl::<BandPass>(freq),
            FilterType::Peaking => self.response_impl::<Peaking>(freq),
            FilterType::Notch => self.response_impl::<Notch>(freq),
        }
    }

    fn response_impl<T: FilterImpl>(&self, freq: Sample) -> ComplexSample {
        let response = T::new(self.gain, self.cutoff_freq, self.q).at(freq);

        if self.linear_phase {
            ComplexSample::new(response.norm(), 0.0)
        } else {
            response
        }
    }

    fn apply_impl<T: FilterImpl>(&self, input: &[ComplexSample], output: &mut [ComplexSample]) {
        let filter_impl = T::new(self.gain, self.cutoff_freq, self.q);

        if self.linear_phase {
            //Skip DC
            for (i, (out, &inp)) in output.iter_mut().zip(input).enumerate().skip(1) {
                *out = inp * filter_impl.at(i as Sample).norm();
            }
        } else {
            //Skip DC
            for (i, (out, &inp)) in output.iter_mut().zip(input).enumerate().skip(1) {
                *out = inp * filter_impl.at(i as Sample);
            }
        }
    }
}
