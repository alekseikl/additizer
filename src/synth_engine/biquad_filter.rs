use std::f32;

use crate::synth_engine::{Sample, buffer::SPECTRAL_BUFFER_SIZE, types::ComplexSample};

const HARMONICS_NUM: usize = SPECTRAL_BUFFER_SIZE - 1;
const TAU: Sample = f32::consts::TAU;

pub struct BiquadFilter {
    gain: Sample,
    cutoff: Sample,
    q: Sample,
}

impl BiquadFilter {
    pub fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self { gain, cutoff, q }
    }

    pub fn low_pass(&self) -> impl Iterator<Item = ComplexSample> + 'static {
        let w = self.cutoff * TAU;
        let w_squared = w * w;
        let w_q = w / self.q;
        let numerator = self.gain * w_squared;

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample * TAU;

            numerator / ComplexSample::new(w_squared - x * x, w_q * x)
        })
    }

    pub fn high_pass(&self) -> impl Iterator<Item = ComplexSample> + 'static {
        let w = self.cutoff * TAU;
        let neg_g = -self.gain;
        let w_squared = w * w;
        let w_q = w / self.q;

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample * TAU;
            let x_squared = x * x;

            (neg_g * x_squared) / ComplexSample::new(w_squared - x_squared, w_q * x)
        })
    }

    pub fn peaking(&self) -> impl Iterator<Item = ComplexSample> + 'static {
        let w = self.cutoff * TAU;
        let a = self.gain;
        let w_squared = w * w;
        let wa_q = (w * a) / self.q;
        let w_aq = w / (a * self.q);

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample * TAU;
            let wx_diff = w_squared - x * x;

            ComplexSample::new(wx_diff, wa_q * x) / ComplexSample::new(wx_diff, w_aq * x)
        })
    }

    pub fn band_pass(&self) -> impl Iterator<Item = ComplexSample> + 'static {
        let a = self.gain;
        let w = self.cutoff * TAU;
        let w_squared = w * w;
        let w_q = w / self.q;

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample * TAU;
            let x_squared = x * x;
            let wx_q = w_q * x;

            ComplexSample::new(0.0, a * wx_q) / ComplexSample::new(w_squared - x_squared, wx_q)
        })
    }

    pub fn band_stop(&self) -> impl Iterator<Item = ComplexSample> + 'static {
        let a = self.gain;
        let w = self.cutoff * TAU;
        let w_squared = w * w;
        let w_q = w / self.q;

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample * TAU;
            let wx_diff = w_squared - x * x;

            (a * wx_diff) / ComplexSample::new(wx_diff, w_q * x)
        })
    }
}
