use realfft::num_complex::Complex;

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
