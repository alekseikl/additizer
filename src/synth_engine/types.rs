use realfft::num_complex::Complex;
use wide::f32x4;

use crate::synth_engine::buffer::{SpectralBuffer, zero_spectral_buffer};

pub type Sample = f32;
pub type ComplexSample = Complex<Sample>;

#[derive(Default)]
pub struct ScalarOutput {
    output: [Sample; 2],
}

impl ScalarOutput {
    pub fn previous(&self) -> Sample {
        self.output[0]
    }

    pub fn current(&self) -> Sample {
        self.output[1]
    }

    pub fn get(&self, current: bool) -> Sample {
        self.output[current as usize]
    }

    pub fn advance(&mut self, next: Sample) {
        self.output[0] = self.output[1];
        self.output[1] = next;
    }
}

pub struct SpectralOutput {
    swapped: bool,
    output: [SpectralBuffer; 2],
}

impl Default for SpectralOutput {
    fn default() -> Self {
        Self {
            swapped: false,
            output: [zero_spectral_buffer(), zero_spectral_buffer()],
        }
    }
}

impl SpectralOutput {
    pub fn get(&self, current: bool) -> &SpectralBuffer {
        &self.output[(current ^ self.swapped) as usize]
    }

    pub fn advance(&mut self) -> &mut SpectralBuffer {
        self.swapped = !self.swapped;
        &mut self.output[!self.swapped as usize]
    }
}

pub struct SimdIter<I> {
    inner: I,
}

impl<I: Iterator<Item = f32>> Iterator for SimdIter<I> {
    type Item = f32x4;

    #[inline]
    fn next(&mut self) -> Option<f32x4> {
        let a = self.inner.next()?;
        let b = self.inner.next().unwrap_or(0.0);
        let c = self.inner.next().unwrap_or(0.0);
        let d = self.inner.next().unwrap_or(0.0);

        Some(f32x4::new([a, b, c, d]))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lo, hi) = self.inner.size_hint();

        (lo.div_ceil(4), hi.map(|h| h.div_ceil(4)))
    }
}

pub trait IntoSimdIter: Iterator<Item = f32> + Sized {
    fn simd(self) -> SimdIter<Self> {
        SimdIter { inner: self }
    }
}

impl<I: Iterator<Item = f32>> IntoSimdIter for I {}
