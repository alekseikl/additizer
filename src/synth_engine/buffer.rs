use core::f32;

use realfft::num_complex::Complex;
use uniform_cubic_splines::{CatmullRom, spline_segment};

pub const BUFFER_SIZE: usize = 128;
pub const WAVE_BITS: usize = 11;
pub const WAVE_SIZE: usize = 1 << WAVE_BITS;
pub const WAVE_PAD_LEFT: usize = 1;
pub const WAVE_PAD_RIGHT: usize = 2;
pub const WAVE_BUFFER_SIZE: usize = WAVE_SIZE + WAVE_PAD_LEFT + WAVE_PAD_RIGHT;
pub const SPECTRAL_BUFFER_SIZE: usize = (1 << (WAVE_BITS - 1)) + 1;

pub type Sample = f32;
pub type ComplexSample = Complex<Sample>;
pub type Buffer = [Sample; BUFFER_SIZE];
pub type WaveBuffer = [Sample; WAVE_BUFFER_SIZE];
pub type SpectralBuffer = [ComplexSample; SPECTRAL_BUFFER_SIZE];

pub const ZEROES_BUFFER: Buffer = [0.0; BUFFER_SIZE];
pub const ONES_BUFFER: Buffer = [1.0; BUFFER_SIZE];
pub const HARMONIC_SERIES_BUFFER: SpectralBuffer = make_harmonic_series_buffer();

pub const fn make_zero_buffer() -> Buffer {
    [0.0; BUFFER_SIZE]
}

pub const fn make_zero_wave_buffer() -> WaveBuffer {
    [0.0; WAVE_BUFFER_SIZE]
}

pub const fn make_zero_spectral_buffer() -> SpectralBuffer {
    [ComplexSample::ZERO; SPECTRAL_BUFFER_SIZE]
}

pub const fn make_harmonic_series_buffer() -> SpectralBuffer {
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

#[inline(always)]
pub fn get_wave_slice_mut(wave_buff: &mut WaveBuffer) -> &mut [Sample] {
    &mut wave_buff[WAVE_PAD_LEFT..(WAVE_BUFFER_SIZE - WAVE_PAD_RIGHT)]
}

#[inline(always)]
pub fn get_interpolated_sample(wave_buff: &WaveBuffer, idx: usize, t: Sample) -> Sample {
    spline_segment::<CatmullRom, _, _>(
        t,
        &wave_buff[(idx - WAVE_PAD_LEFT)..(idx + WAVE_PAD_RIGHT + 1)],
    )
}

#[inline(always)]
pub fn wrap_wave_buffer(wave_buff: &mut WaveBuffer) {
    wave_buff[0] = wave_buff[WAVE_BUFFER_SIZE - WAVE_PAD_RIGHT - 1];
    wave_buff[WAVE_BUFFER_SIZE - WAVE_PAD_RIGHT] = wave_buff[WAVE_PAD_LEFT];
    wave_buff[WAVE_BUFFER_SIZE - WAVE_PAD_RIGHT + 1] = wave_buff[WAVE_PAD_LEFT + 1];
}
