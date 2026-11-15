use crate::synth_engine::{
    Buffer, ComplexSample, Smp, SpectralBuffer,
    buffer::{zero_buffer, zero_spectral_buffer},
};

pub struct SamplesOutput {
    pub(super) buffer: Buffer,
}

impl SamplesOutput {
    pub(crate) fn new(len: usize) -> Self {
        Self {
            buffer: zero_buffer(len),
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

    /// `Some(offset)` is that sample. `None` is the last sample of the block,
    /// which spectral modules use as the next frame.
    pub(super) fn scalar(&self, this_frame: Option<usize>, block_samples: usize) -> Smp {
        let index = this_frame.unwrap_or(block_samples - 1);

        self.buffer[index]
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
