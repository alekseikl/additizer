use std::f32;

use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ComplexSample, Sample},
    utils::db_to_gain_fast,
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
    butterworth: LowPass12,
    resonant: LowPass12,
}

impl FilterImpl for LowPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            butterworth: LowPass12::new(1.0, cutoff, BUTTERWORTH_Q),
            resonant: LowPass12::new(gain, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        self.butterworth.at(freq) * self.resonant.at(freq)
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
    butterworth: HighPass12,
    resonant: HighPass12,
}

impl FilterImpl for HighPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            butterworth: HighPass12::new(1.0, cutoff, BUTTERWORTH_Q),
            resonant: HighPass12::new(gain, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        self.butterworth.at(freq) * self.resonant.at(freq)
    }
}

#[derive(Clone, Copy)]
pub struct BandPass6 {
    gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl FilterImpl for BandPass6 {
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
pub struct BandPass12 {
    butterworth: BandPass6,
    resonant: BandPass6,
}

impl FilterImpl for BandPass12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            butterworth: BandPass6::new(1.0, cutoff, BUTTERWORTH_Q),
            resonant: BandPass6::new(gain, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        self.butterworth.at(freq) * self.resonant.at(freq)
    }
}

#[derive(Clone, Copy)]
pub struct BandPass18 {
    butterworth: BandPass6,
    resonant: BandPass6,
}

impl FilterImpl for BandPass18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            butterworth: BandPass6::new(1.0, cutoff, BUTTERWORTH_Q),
            resonant: BandPass6::new(gain, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let butterworth = self.butterworth.at(freq);

        butterworth * butterworth * self.resonant.at(freq)
    }
}

#[derive(Clone, Copy)]
pub struct BandPass24 {
    butterworth: BandPass6,
    resonant: BandPass6,
}

impl FilterImpl for BandPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            butterworth: BandPass6::new(1.0, cutoff, BUTTERWORTH_Q),
            resonant: BandPass6::new(gain, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let butterworth = self.butterworth.at(freq);
        let butterworth_sq = butterworth * butterworth;

        butterworth_sq * butterworth * self.resonant.at(freq)
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
        let a = gain.max(0.0).sqrt();

        Self {
            w_squared: w * w,
            wa_q: (w * a) / q,
            w_aq: w / (a * q),
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
pub struct LowShelf12 {
    coeffs: ShelfCoeffs,
}

impl FilterImpl for LowShelf12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            coeffs: ShelfCoeffs::new(gain, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_sq = x * x;
        let j_damp = self.coeffs.w_damp * x;
        let c = &self.coeffs;

        c.a * ComplexSample::new(c.a_w_sq - x_sq, j_damp)
            / ComplexSample::new(c.w_sq - c.a * x_sq, j_damp)
    }
}

#[derive(Clone, Copy)]
pub struct HighShelf12 {
    coeffs: ShelfCoeffs,
}

impl FilterImpl for HighShelf12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self {
            coeffs: ShelfCoeffs::new(gain, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_sq = x * x;
        let j_damp = self.coeffs.w_damp * x;
        let c = &self.coeffs;

        c.a * ComplexSample::new(c.w_sq - c.a * x_sq, j_damp)
            / ComplexSample::new(c.a_w_sq - x_sq, j_damp)
    }
}

#[derive(Clone, Copy)]
struct LowShelf6 {
    w_sqrt_a: Sample,
    w_inv_sqrt_a: Sample,
}

impl FilterImpl for LowShelf6 {
    fn new(gain: Sample, cutoff: Sample, _q: Sample) -> Self {
        let w = cutoff * TAU;
        let sqrt_a = gain.max(0.0).sqrt();

        Self {
            w_sqrt_a: w * sqrt_a,
            w_inv_sqrt_a: w / sqrt_a,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;

        ComplexSample::new(self.w_sqrt_a, x) / ComplexSample::new(self.w_inv_sqrt_a, x)
    }
}

#[derive(Clone, Copy)]
struct HighShelf6 {
    gain: Sample,
    w_sqrt_a: Sample,
    w_inv_sqrt_a: Sample,
}

impl FilterImpl for HighShelf6 {
    fn new(gain: Sample, cutoff: Sample, _q: Sample) -> Self {
        let w = cutoff * TAU;
        let sqrt_a = gain.max(0.0).sqrt();

        Self {
            gain,
            w_sqrt_a: w * sqrt_a,
            w_inv_sqrt_a: w / sqrt_a,
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;

        self.gain * ComplexSample::new(self.w_inv_sqrt_a, x) / ComplexSample::new(self.w_sqrt_a, x)
    }
}

fn shelf_18_gains(gain: Sample) -> (Sample, Sample) {
    let g6 = gain.max(0.0).cbrt();
    (g6 * g6, g6)
}

#[derive(Clone, Copy)]
pub struct LowShelf18 {
    biquad: LowShelf12,
    one_pole: LowShelf6,
}

impl FilterImpl for LowShelf18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let (g12, g6) = shelf_18_gains(gain);

        Self {
            biquad: LowShelf12::new(g12, cutoff, q),
            one_pole: LowShelf6::new(g6, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        self.one_pole.at(freq) * self.biquad.at(freq)
    }
}

#[derive(Clone, Copy)]
pub struct HighShelf18 {
    biquad: HighShelf12,
    one_pole: HighShelf6,
}

impl FilterImpl for HighShelf18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let (g12, g6) = shelf_18_gains(gain);

        Self {
            biquad: HighShelf12::new(g12, cutoff, q),
            one_pole: HighShelf6::new(g6, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        self.one_pole.at(freq) * self.biquad.at(freq)
    }
}

#[derive(Clone, Copy)]
pub struct LowShelf24 {
    butterworth: LowShelf12,
    resonant: LowShelf12,
}

impl FilterImpl for LowShelf24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let g12 = gain.max(0.0).sqrt();

        Self {
            butterworth: LowShelf12::new(g12, cutoff, BUTTERWORTH_Q),
            resonant: LowShelf12::new(g12, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        self.butterworth.at(freq) * self.resonant.at(freq)
    }
}

#[derive(Clone, Copy)]
pub struct HighShelf24 {
    butterworth: HighShelf12,
    resonant: HighShelf12,
}

impl FilterImpl for HighShelf24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let g12 = gain.max(0.0).sqrt();

        Self {
            butterworth: HighShelf12::new(g12, cutoff, BUTTERWORTH_Q),
            resonant: HighShelf12::new(g12, cutoff, q),
        }
    }

    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        self.butterworth.at(freq) * self.resonant.at(freq)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FilterType {
    #[default]
    LowPass12,
    LowPass18,
    LowPass24,
    #[serde(alias = "LowShelf")]
    LowShelf12,
    LowShelf18,
    LowShelf24,
    HighPass12,
    HighPass18,
    HighPass24,
    #[serde(alias = "HighShelf")]
    HighShelf12,
    HighShelf18,
    HighShelf24,
    #[serde(alias = "BandPass")]
    BandPass6,
    BandPass12,
    BandPass18,
    BandPass24,
    Peaking,
    Notch,
}

impl FilterType {
    pub const ALL: [Self; 18] = [
        Self::LowPass12,
        Self::LowPass18,
        Self::LowPass24,
        Self::LowShelf12,
        Self::LowShelf18,
        Self::LowShelf24,
        Self::HighPass12,
        Self::HighPass18,
        Self::HighPass24,
        Self::HighShelf12,
        Self::HighShelf18,
        Self::HighShelf24,
        Self::BandPass6,
        Self::BandPass12,
        Self::BandPass18,
        Self::BandPass24,
        Self::Peaking,
        Self::Notch,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::LowPass12 => "Lowpass 12",
            Self::LowPass18 => "Lowpass 18",
            Self::LowPass24 => "Lowpass 24",
            Self::LowShelf12 => "Lowshelf 12",
            Self::LowShelf18 => "Lowshelf 18",
            Self::LowShelf24 => "Lowshelf 24",
            Self::HighPass12 => "Highpass 12",
            Self::HighPass18 => "Highpass 18",
            Self::HighPass24 => "Highpass 24",
            Self::HighShelf12 => "Highshelf 12",
            Self::HighShelf18 => "Highshelf 18",
            Self::HighShelf24 => "Highshelf 24",
            Self::BandPass6 => "Bandpass 6",
            Self::BandPass12 => "Bandpass 12",
            Self::BandPass18 => "Bandpass 18",
            Self::BandPass24 => "Bandpass 24",
            Self::Peaking => "Peaking",
            Self::Notch => "Notch",
        }
    }
}

#[derive(Clone, Copy)]
pub struct FilterParams {
    pub drive: Sample,
    pub cutoff: Sample, // Octaves of the note fundamental (harmonic space).
    pub resonance: Sample,
    pub q_limit_to: Sample,    // Octaves. Before this point Q is limited.
    pub q_limit_slope: Sample, // [0.0-1.0]
    pub linear_phase: bool,
}

pub const MIN_RESONANCE: Sample = -1.0;
pub const MAX_RESONANCE: Sample = 1.0;
const MIN_Q: Sample = 0.01;
const MAX_Q: Sample = 16.0;
const MAX_Q_LIMIT_POWER: Sample = 20.0;

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
            gain: db_to_gain_fast(params.drive),
            cutoff_freq: params.cutoff.exp2(),
            q: Self::q_from_params(&params),
            linear_phase: params.linear_phase,
        }
    }

    fn q_from_params(params: &FilterParams) -> Sample {
        let resonance = params.resonance.clamp(MIN_RESONANCE, MAX_RESONANCE);

        let q = if resonance > 0.0 {
            BUTTERWORTH_Q + (MAX_Q - BUTTERWORTH_Q) * resonance.powf(3.0)
        } else {
            MIN_Q + (BUTTERWORTH_Q - MIN_Q) * (1.0 + resonance)
        };

        let butterworth_excess = q - BUTTERWORTH_Q;

        if butterworth_excess <= 0.0 || params.cutoff >= params.q_limit_to {
            return q;
        }

        let rate = 1.0 + params.q_limit_slope.clamp(0.0, 1.0) * MAX_Q_LIMIT_POWER;

        BUTTERWORTH_Q + butterworth_excess * ((params.cutoff - params.q_limit_to) * rate).exp2()
    }

    pub fn apply_response(&self, input: &[ComplexSample], output: &mut [ComplexSample]) {
        match self.filter_type {
            FilterType::LowPass12 => self.apply_impl::<LowPass12>(input, output),
            FilterType::LowPass18 => self.apply_impl::<LowPass18>(input, output),
            FilterType::LowPass24 => self.apply_impl::<LowPass24>(input, output),
            FilterType::LowShelf12 => self.apply_impl::<LowShelf12>(input, output),
            FilterType::LowShelf18 => self.apply_impl::<LowShelf18>(input, output),
            FilterType::LowShelf24 => self.apply_impl::<LowShelf24>(input, output),
            FilterType::HighPass12 => self.apply_impl::<HighPass12>(input, output),
            FilterType::HighPass18 => self.apply_impl::<HighPass18>(input, output),
            FilterType::HighPass24 => self.apply_impl::<HighPass24>(input, output),
            FilterType::HighShelf12 => self.apply_impl::<HighShelf12>(input, output),
            FilterType::HighShelf18 => self.apply_impl::<HighShelf18>(input, output),
            FilterType::HighShelf24 => self.apply_impl::<HighShelf24>(input, output),
            FilterType::BandPass6 => self.apply_impl::<BandPass6>(input, output),
            FilterType::BandPass12 => self.apply_impl::<BandPass12>(input, output),
            FilterType::BandPass18 => self.apply_impl::<BandPass18>(input, output),
            FilterType::BandPass24 => self.apply_impl::<BandPass24>(input, output),
            FilterType::Peaking => self.apply_impl::<Peaking>(input, output),
            FilterType::Notch => self.apply_impl::<Notch>(input, output),
        }
    }

    pub fn response_at_freqs(&self, freqs: &[Sample], out: &mut [ComplexSample]) {
        match self.filter_type {
            FilterType::LowPass12 => self.response_freqs_impl::<LowPass12>(freqs, out),
            FilterType::LowPass18 => self.response_freqs_impl::<LowPass18>(freqs, out),
            FilterType::LowPass24 => self.response_freqs_impl::<LowPass24>(freqs, out),
            FilterType::LowShelf12 => self.response_freqs_impl::<LowShelf12>(freqs, out),
            FilterType::LowShelf18 => self.response_freqs_impl::<LowShelf18>(freqs, out),
            FilterType::LowShelf24 => self.response_freqs_impl::<LowShelf24>(freqs, out),
            FilterType::HighPass12 => self.response_freqs_impl::<HighPass12>(freqs, out),
            FilterType::HighPass18 => self.response_freqs_impl::<HighPass18>(freqs, out),
            FilterType::HighPass24 => self.response_freqs_impl::<HighPass24>(freqs, out),
            FilterType::HighShelf12 => self.response_freqs_impl::<HighShelf12>(freqs, out),
            FilterType::HighShelf18 => self.response_freqs_impl::<HighShelf18>(freqs, out),
            FilterType::HighShelf24 => self.response_freqs_impl::<HighShelf24>(freqs, out),
            FilterType::BandPass6 => self.response_freqs_impl::<BandPass6>(freqs, out),
            FilterType::BandPass12 => self.response_freqs_impl::<BandPass12>(freqs, out),
            FilterType::BandPass18 => self.response_freqs_impl::<BandPass18>(freqs, out),
            FilterType::BandPass24 => self.response_freqs_impl::<BandPass24>(freqs, out),
            FilterType::Peaking => self.response_freqs_impl::<Peaking>(freqs, out),
            FilterType::Notch => self.response_freqs_impl::<Notch>(freqs, out),
        }
    }

    fn response_freqs_impl<T: FilterImpl>(&self, freqs: &[Sample], out: &mut [ComplexSample]) {
        let filter_impl = T::new(self.gain, self.cutoff_freq, self.q);

        if self.linear_phase {
            for (out, &freq) in out.iter_mut().zip(freqs) {
                *out = ComplexSample::new(filter_impl.at(freq).norm(), 0.0);
            }
        } else {
            for (out, &freq) in out.iter_mut().zip(freqs) {
                *out = filter_impl.at(freq);
            }
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

#[cfg(test)]
mod tests;
