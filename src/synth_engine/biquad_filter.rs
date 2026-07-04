use std::f32;

use crate::synth_engine::{Sample, types::ComplexSample};

const TAU: Sample = f32::consts::TAU;

pub trait FilterImpl: Clone + Copy + 'static {
    fn at(&self, freq: Sample) -> ComplexSample;

    fn into_iter(self, size: usize) -> impl Iterator<Item = ComplexSample> + 'static {
        (0..size).map(move |i| self.at(i as Sample))
    }
}

#[derive(Clone, Copy)]
pub struct LowPass {
    numerator: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl LowPass {
    pub fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;
        let w_squared = w * w;

        Self {
            numerator: gain * w_squared,
            w_squared,
            w_q: w / q,
        }
    }
}

impl FilterImpl for LowPass {
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;

        self.numerator / ComplexSample::new(self.w_squared - x * x, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub struct HighPass {
    neg_gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl HighPass {
    pub fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            neg_gain: -gain,
            w_squared: w * w,
            w_q: w / q,
        }
    }
}

impl FilterImpl for HighPass {
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let x_squared = x * x;

        (self.neg_gain * x_squared) / ComplexSample::new(self.w_squared - x_squared, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub struct Peaking {
    w_squared: Sample,
    wa_q: Sample,
    w_aq: Sample,
}

impl Peaking {
    pub fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            w_squared: w * w,
            wa_q: (w * gain) / q,
            w_aq: w / (gain * q),
        }
    }
}

impl FilterImpl for Peaking {
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let wx_diff = self.w_squared - x * x;

        ComplexSample::new(wx_diff, self.wa_q * x) / ComplexSample::new(wx_diff, self.w_aq * x)
    }
}

#[derive(Clone, Copy)]
pub struct BandPass {
    gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl BandPass {
    pub fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            gain,
            w_squared: w * w,
            w_q: w / q,
        }
    }
}

impl FilterImpl for BandPass {
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let wx_q = self.w_q * x;

        ComplexSample::new(0.0, self.gain * wx_q) / ComplexSample::new(self.w_squared - x * x, wx_q)
    }
}

#[derive(Clone, Copy)]
pub struct BandStop {
    gain: Sample,
    w_squared: Sample,
    w_q: Sample,
}

impl BandStop {
    pub fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        let w = cutoff * TAU;

        Self {
            gain,
            w_squared: w * w,
            w_q: w / q,
        }
    }
}

impl FilterImpl for BandStop {
    fn at(&self, freq: Sample) -> ComplexSample {
        let x = freq * TAU;
        let wx_diff = self.w_squared - x * x;

        (self.gain * wx_diff) / ComplexSample::new(wx_diff, self.w_q * x)
    }
}

#[derive(Clone, Copy)]
pub enum FilterPole {
    Pole2,
    Pole3,
    Pole4,
}

pub struct BiquadParams {
    pub cutoff: Sample,
    pub q: Sample,
    pub gain: Sample,
    pub pole: FilterPole,
    pub linear_phase: bool,
}

pub struct Biquad<T: FilterImpl> {
    filter_impl: T,
    pole: FilterPole,
    linear_phase: bool,
}

impl Biquad<LowPass> {
    pub fn new(params: &BiquadParams) -> Self {
        Self {
            filter_impl: LowPass::new(params.gain, params.cutoff, params.q),
            pole: params.pole,
            linear_phase: params.linear_phase,
        }
    }
}

impl Biquad<HighPass> {
    pub fn new(params: &BiquadParams) -> Self {
        Self {
            filter_impl: HighPass::new(params.gain, params.cutoff, params.q),
            pole: params.pole,
            linear_phase: params.linear_phase,
        }
    }
}

impl Biquad<Peaking> {
    pub fn new(params: &BiquadParams) -> Self {
        Self {
            filter_impl: Peaking::new(params.gain, params.cutoff, params.q),
            pole: params.pole,
            linear_phase: params.linear_phase,
        }
    }
}

impl Biquad<BandPass> {
    pub fn new(params: &BiquadParams) -> Self {
        Self {
            filter_impl: BandPass::new(params.gain, params.cutoff, params.q),
            pole: params.pole,
            linear_phase: params.linear_phase,
        }
    }
}

impl Biquad<BandStop> {
    pub fn new(params: &BiquadParams) -> Self {
        Self {
            filter_impl: BandStop::new(params.gain, params.cutoff, params.q),
            pole: params.pole,
            linear_phase: params.linear_phase,
        }
    }
}

impl<T: FilterImpl> Biquad<T> {
    fn apply_2_pole(&self, freq: Sample, input: &ComplexSample) -> ComplexSample {
        input * self.filter_impl.at(freq)
    }

    fn apply_2_pole_linear(&self, freq: Sample, input: &ComplexSample) -> ComplexSample {
        input * self.filter_impl.at(freq).norm()
    }

    fn apply_3_pole(&self, freq: Sample, input: &ComplexSample) -> ComplexSample {
        input * self.filter_impl.at(freq).powf(1.5)
    }

    fn apply_3_pole_linear(&self, freq: Sample, input: &ComplexSample) -> ComplexSample {
        input * self.filter_impl.at(freq).norm().powf(1.5)
    }

    fn apply_4_pole(&self, freq: Sample, input: &ComplexSample) -> ComplexSample {
        let response = self.filter_impl.at(freq);

        input * response * response
    }

    fn apply_4_pole_linear(&self, freq: Sample, input: &ComplexSample) -> ComplexSample {
        let response = self.filter_impl.at(freq).norm();

        input * response * response
    }

    fn apply_impl<'a>(
        &self,
        input: impl Iterator<Item = &'a ComplexSample>,
        output: impl Iterator<Item = &'a mut ComplexSample>,
        f: impl Fn(&Self, Sample, &ComplexSample) -> ComplexSample,
    ) {
        for (i, (output, input)) in output.zip(input).enumerate() {
            *output = f(self, i as Sample, input);
        }
    }

    pub fn apply_response<'a>(
        &self,
        input: impl Iterator<Item = &'a ComplexSample>,
        output: impl Iterator<Item = &'a mut ComplexSample>,
    ) {
        match self.pole {
            FilterPole::Pole2 => {
                if self.linear_phase {
                    self.apply_impl(input, output, Self::apply_2_pole_linear)
                } else {
                    self.apply_impl(input, output, Self::apply_2_pole)
                }
            }
            FilterPole::Pole3 => {
                if self.linear_phase {
                    self.apply_impl(input, output, Self::apply_3_pole_linear)
                } else {
                    self.apply_impl(input, output, Self::apply_3_pole)
                }
            }
            FilterPole::Pole4 => {
                if self.linear_phase {
                    self.apply_impl(input, output, Self::apply_4_pole_linear)
                } else {
                    self.apply_impl(input, output, Self::apply_4_pole)
                }
            }
        }
    }

    pub fn response_at(&self, freq: Sample) -> ComplexSample {
        let one = ComplexSample::new(1.0, 0.0);

        match self.pole {
            FilterPole::Pole2 => {
                if self.linear_phase {
                    self.apply_2_pole_linear(freq, &one)
                } else {
                    self.apply_2_pole(freq, &one)
                }
            }
            FilterPole::Pole3 => {
                if self.linear_phase {
                    self.apply_3_pole_linear(freq, &one)
                } else {
                    self.apply_3_pole(freq, &one)
                }
            }
            FilterPole::Pole4 => {
                if self.linear_phase {
                    self.apply_4_pole_linear(freq, &one)
                } else {
                    self.apply_4_pole(freq, &one)
                }
            }
        }
    }
}
