use core::f32;

use uniform_cubic_splines::{CatmullRom, spline_segment};

use crate::synth_engine::types::{ComplexSample, Sample};

pub const BUFFER_SIZE: usize = 128;
pub const WAVEFORM_BITS: usize = 11;
pub const WAVEFORM_SIZE: usize = 1 << WAVEFORM_BITS;
pub const WAVEFORM_PAD_LEFT: usize = 1;
pub const WAVEFORM_PAD_RIGHT: usize = 2;
pub const WAVEFORM_BUFFER_SIZE: usize = WAVEFORM_SIZE + WAVEFORM_PAD_LEFT + WAVEFORM_PAD_RIGHT;
pub const SPECTRAL_BUFFER_SIZE: usize = (1 << (WAVEFORM_BITS - 1)) + 1;

pub type Buffer = [Sample; BUFFER_SIZE];
pub type WaveformBuffer = [Sample; WAVEFORM_BUFFER_SIZE];
pub type SpectralBuffer = [ComplexSample; SPECTRAL_BUFFER_SIZE];

pub const ZEROES_BUFFER: Buffer = [0.0; BUFFER_SIZE];
pub const ONES_BUFFER: Buffer = [1.0; BUFFER_SIZE];
// pub const ZEROES_SPECTRAL_BUFFER: SpectralBuffer = make_zero_spectral_buffer();
pub const HARMONIC_SERIES_BUFFER: SpectralBuffer = make_harmonic_series_buffer();

pub const fn make_zero_buffer() -> Buffer {
    [0.0; BUFFER_SIZE]
}

pub const fn make_zero_wave_buffer() -> WaveformBuffer {
    [0.0; WAVEFORM_BUFFER_SIZE]
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
pub fn get_wave_slice_mut(wave_buff: &mut WaveformBuffer) -> &mut [Sample] {
    &mut wave_buff[WAVEFORM_PAD_LEFT..(WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT)]
}

#[inline(always)]
pub fn get_interpolated_sample(wave_buff: &WaveformBuffer, idx: usize, t: Sample) -> Sample {
    spline_segment::<CatmullRom, _, _>(
        t,
        &wave_buff[idx..(idx + WAVEFORM_PAD_LEFT + WAVEFORM_PAD_RIGHT + 1)],
    )
}

#[inline(always)]
pub fn wrap_wave_buffer(wave_buff: &mut WaveformBuffer) {
    wave_buff[0] = wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT - 1];
    wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT] = wave_buff[WAVEFORM_PAD_LEFT];
    wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT + 1] = wave_buff[WAVEFORM_PAD_LEFT + 1];
}
