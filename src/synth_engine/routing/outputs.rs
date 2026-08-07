use crate::synth_engine::{
    Buffer, Sample, SpectralBuffer,
    buffer::{zero_buffer, zero_spectral_buffer},
};

pub struct SamplesOutput {
    pub(super) buffer: Buffer,
    pub(super) next_frame_sample: Sample,
}

impl SamplesOutput {
    #[inline]
    pub(super) fn buffer(&self) -> &[Sample] {
        &self.buffer
    }

    pub(super) fn scalar(&self, this_frame: Option<usize>) -> Sample {
        if let Some(offset) = this_frame {
            self.buffer[offset]
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
    pub fn fill_with_ext_control(&mut self, offset: usize, in_buff: &[Sample]) {
        let len = in_buff.len();
        let last = in_buff[len - 1];

        self.buffer[offset..offset + len].copy_from_slice(in_buff);
        self.buffer[offset + len] = last;
        self.next_frame_sample = last;
    }

    pub fn fill_with_ext_control_value(&mut self, offset: usize, samples: usize, value: Sample) {
        self.buffer[offset..samples + 1].fill(value);
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
    pub(super) fn get(&self, this_frame: bool) -> &SpectralBuffer {
        &self.output[(!this_frame ^ self.swapped) as usize]
    }

    pub(super) fn buff(&mut self) -> &mut SpectralBuffer {
        &mut self.output[self.swapped as usize]
    }

    pub fn advance(&mut self) -> &mut SpectralBuffer {
        self.swapped = !self.swapped;
        &mut self.output[!self.swapped as usize]
    }
}
