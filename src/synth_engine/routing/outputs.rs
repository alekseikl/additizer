use crate::synth_engine::{
    Buffer, Sample, SpectralBuffer,
    buffer::{zero_buffer, zero_spectral_buffer},
};

pub struct SamplesOutput {
    buffer: Buffer,
    next_frame_sample: Sample,
}

impl SamplesOutput {
    #[inline]
    pub(super) fn buffer(&self) -> &[Sample] {
        &self.buffer
    }

    pub(super) fn scalar(&self, triggered: bool) -> Sample {
        if triggered {
            self.buffer[0]
        } else {
            self.next_frame_sample
        }
    }

    pub fn output(&mut self, samples: usize) -> &mut [Sample] {
        &mut self.buffer[..samples]
    }

    pub fn control_output(&mut self, samples: usize, triggered: bool) -> ControlRateAdapter<'_> {
        ControlRateAdapter {
            output: self,
            samples,
            triggered,
        }
    }

    // Fill buffer with the external control-rate signal that doesn't run 1 sample ahead.
    pub fn fill_with_ext_control(&mut self, buff: &[Sample]) {
        let len = buff.len();
        let last = buff[len - 1];

        self.buffer[..len].copy_from_slice(buff);
        self.buffer[len] = last;
        self.next_frame_sample = last;
    }

    pub fn fill_with_ext_control_value(&mut self, samples: usize, value: Sample) {
        self.buffer[..samples + 1].fill(value);
        self.next_frame_sample = value;
    }
}

impl Default for SamplesOutput {
    fn default() -> Self {
        Self {
            buffer: zero_buffer(),
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

    pub fn next_frame_value(&self) -> Sample {
        self.output.buffer[self.samples]
    }
}

impl<'a> Drop for ControlRateAdapter<'a> {
    fn drop(&mut self) {
        if !self.triggered {
            self.output.buffer[0] = self.output.next_frame_sample;
        }

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
    pub(super) fn get(&self, triggered: bool) -> &SpectralBuffer {
        &self.output[(!triggered ^ self.swapped) as usize]
    }

    pub fn advance(&mut self) -> &mut SpectralBuffer {
        self.swapped = !self.swapped;
        &mut self.output[!self.swapped as usize]
    }
}
