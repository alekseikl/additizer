use crate::synth_engine::{
    Buffer, ComplexSample, Smp, SpectralBuffer,
    buffer::{zero_buffer, zero_spectral_buffer},
};

pub struct SamplesOutput {
    pub(super) buffer: Buffer,
    pub(super) next_frame_sample: Smp,
}

impl SamplesOutput {
    pub(crate) fn new(len: usize) -> Self {
        Self {
            buffer: zero_buffer(len),
            next_frame_sample: 0.0,
        }
    }

    #[inline]
    pub(super) fn buffer(&self) -> &[Smp] {
        &self.buffer
    }

    #[inline]
    pub(crate) fn buffer_mut(&mut self) -> &mut [Smp] {
        &mut self.buffer
    }

    pub(super) fn scalar(&self, this_frame: Option<usize>) -> Smp {
        if let Some(offset) = this_frame {
            self.buffer[offset]
        } else {
            self.next_frame_sample
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
