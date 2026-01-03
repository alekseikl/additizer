use core::f32;

use crate::synth_engine::types::{ComplexSample, Sample};

pub const BUFFER_SIZE: usize = 128;
pub const SPECTRUM_BITS: usize = 10;
pub const SPECTRAL_BUFFER_SIZE: usize = 1 << SPECTRUM_BITS;

pub type Buffer = [Sample; BUFFER_SIZE];
pub type SpectralBuffer = [ComplexSample; SPECTRAL_BUFFER_SIZE];

pub static ZEROES_BUFFER: Buffer = [0.0; BUFFER_SIZE];
pub static ONES_BUFFER: Buffer = [1.0; BUFFER_SIZE];
pub static ZEROES_SPECTRAL_BUFFER: SpectralBuffer = zero_spectral_buffer();
pub static HARMONIC_SERIES_BUFFER: SpectralBuffer = harmonic_series_buffer();

pub const fn zero_buffer() -> Buffer {
    [0.0; BUFFER_SIZE]
}

pub const fn zero_spectral_buffer() -> SpectralBuffer {
    [ComplexSample::ZERO; SPECTRAL_BUFFER_SIZE]
}

pub const fn harmonic_series_buffer() -> SpectralBuffer {
    let mut buff: SpectralBuffer = [ComplexSample::ZERO; SPECTRAL_BUFFER_SIZE];
    let mut i = 1;

    while i < SPECTRAL_BUFFER_SIZE {
        buff[i].im = -1.0 / (i as f32 * f32::consts::PI);

        if i % 2 == 0 {
            buff[i].im = -buff[i].im;
        }

        i += 1;
    }

    buff
}

pub fn copy_buffer_slice(buff: &mut [Sample], iter: impl Iterator<Item = Sample>) {
    buff.iter_mut()
        .zip(iter)
        .for_each(|(buff, value)| *buff = value);
}

pub fn add_buffer_slice(buff: &mut [Sample], iter: impl Iterator<Item = Sample>) {
    buff.iter_mut()
        .zip(iter)
        .for_each(|(buff, value)| *buff += value);
}

pub fn copy_or_add_buffer(copy: bool, buff: &mut [Sample], input: impl Iterator<Item = Sample>) {
    if copy {
        copy_buffer_slice(buff, input);
    } else {
        add_buffer_slice(buff, input);
    }
}
