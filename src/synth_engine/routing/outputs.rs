use crate::synth_engine::{
    Buffer, ComplexSample, Sample, SpectralBuffer,
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
    length: usize,
    output: SpectralBuffer,
}

impl Default for SpectralOutput {
    fn default() -> Self {
        Self {
            length: 0,
            output: zero_spectral_buffer(),
        }
    }
}

impl SpectralOutput {
    pub(super) fn get(&self) -> &[ComplexSample] {
        &self.output[..self.length]
    }

    pub(super) fn get_mut(&mut self) -> &mut [ComplexSample] {
        &mut self.output[..self.length]
    }

    pub(super) fn set_length(&mut self, new_length: usize) {
        self.length = new_length.min(self.output.len());
    }
}
