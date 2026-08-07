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
}

impl Default for SamplesOutput {
    fn default() -> Self {
        Self {
            buffer: zero_buffer(),
            next_frame_sample: 0.0,
        }
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

    pub fn advance(&mut self) {
        self.swapped = !self.swapped;
    }
}
