use realfft::num_complex::Complex;

use crate::synth_engine::{
    Buffer,
    buffer::{SpectralBuffer, zero_buffer, zero_spectral_buffer},
};

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

pub struct SamplesOutput {
    buffer: Buffer,
    this_frame_sample: Sample,
    next_frame_sample: Sample,
}

impl SamplesOutput {
    #[inline]
    pub fn buffer(&self) -> &[Sample] {
        &self.buffer
    }

    pub fn scalar(&self, triggered: bool) -> Sample {
        if triggered {
            self.this_frame_sample
        } else {
            self.next_frame_sample
        }
    }

    pub fn output(&mut self) -> &mut [Sample] {
        &mut self.buffer
    }

    pub fn control_output(&mut self, samples: usize, triggered: bool) -> ControlRateAdapter<'_> {
        ControlRateAdapter {
            output: self,
            samples,
            triggered,
        }
    }
}

impl Default for SamplesOutput {
    fn default() -> Self {
        Self {
            buffer: zero_buffer(),
            this_frame_sample: 0.0,
            next_frame_sample: 0.0,
        }
    }
}

pub struct ControlRateAdapter<'a> {
    output: &'a mut SamplesOutput,
    samples: usize,
    triggered: bool,
}

impl<'a> ControlRateAdapter<'a> {
    pub fn output(&mut self) -> &mut [Sample] {
        let from = if self.triggered { 0 } else { 1 };

        &mut self.output.buffer[from..self.samples + 1]
    }
}

impl<'a> Drop for ControlRateAdapter<'a> {
    fn drop(&mut self) {
        if !self.triggered {
            self.output.buffer[0] = self.output.next_frame_sample;
        }

        self.output.this_frame_sample = self.output.buffer[0];
        self.output.next_frame_sample = self.output.buffer[self.samples];
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
    pub fn get(&self, triggered: bool) -> &SpectralBuffer {
        &self.output[(!triggered ^ self.swapped) as usize]
    }

    pub fn advance(&mut self) -> &mut SpectralBuffer {
        self.swapped = !self.swapped;
        &mut self.output[!self.swapped as usize]
    }
}
