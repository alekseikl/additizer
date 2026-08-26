use core::f32;
use std::mem::MaybeUninit;

use crate::synth_engine::{
    routing::{MAX_VOICES, NUM_CHANNELS},
    types::{ComplexSample, Sample},
};

// One sample extra for control rate signals. Its value - first sample of the next frame.
// Required by spectral module inputs.
pub const BUFFER_SIZE: usize = 256 + 1;
pub const SPECTRUM_BITS: usize = 10;
pub const SPECTRAL_BUFFER_SIZE: usize = 1 << SPECTRUM_BITS;
pub const DISPLAY_SPECTRUM_SIZE: usize = 512;
pub const DC_OFFSET: usize = 1;

pub type Buffer = [Sample; BUFFER_SIZE];
pub type SpectralBuffer = [ComplexSample; SPECTRAL_BUFFER_SIZE];
pub type DisplaySpectrum = [ComplexSample; DISPLAY_SPECTRUM_SIZE];

pub static ZEROES_BUFFER: Buffer = [0.0; BUFFER_SIZE];
#[allow(unused)]
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

pub type VoicesLayoutArray<T> = [[T; MAX_VOICES]; NUM_CHANNELS];
pub type VoicesLayout<T> = Box<VoicesLayoutArray<T>>;

pub type MonoVoicesLayoutArray<T> = [T; MAX_VOICES];
pub type MonoVoicesLayout<T> = Box<MonoVoicesLayoutArray<T>>;

pub fn new_voices_layout<U: Default + Send>() -> VoicesLayout<U> {
    let mut channels: Box<[MaybeUninit<[U; MAX_VOICES]>; NUM_CHANNELS]> =
        Box::new([const { MaybeUninit::uninit() }; NUM_CHANNELS]);

    for channel in channels.iter_mut() {
        init_array_in_place::<U, MAX_VOICES>(channel.as_mut_ptr());
    }

    unsafe { Box::from_raw(Box::into_raw(channels).cast::<[[U; MAX_VOICES]; NUM_CHANNELS]>()) }
}

pub fn new_mono_voices_layout<U: Default + Send>() -> MonoVoicesLayout<U> {
    let mut voices: Box<MaybeUninit<[U; MAX_VOICES]>> = Box::new(MaybeUninit::uninit());
    init_array_in_place::<U, MAX_VOICES>(voices.as_mut_ptr());
    unsafe { Box::from_raw(Box::into_raw(voices).cast::<[U; MAX_VOICES]>()) }
}

fn init_array_in_place<U: Default, const N: usize>(dst: *mut [U; N]) {
    let elements = dst.cast::<U>();

    for i in 0..N {
        unsafe {
            elements.add(i).write(U::default());
        }
    }
}

pub struct ValueBuffer {
    change_at: usize,
    buffer: Buffer,
}

impl Default for ValueBuffer {
    fn default() -> Self {
        Self {
            change_at: 0,
            buffer: zero_buffer(),
        }
    }
}

impl ValueBuffer {
    pub fn set(&mut self, value: Sample, at: usize) {
        if at > self.change_at {
            let prev = self.buffer[self.change_at];
            self.buffer[self.change_at + 1..at].fill(prev);
        }

        self.buffer[at] = value;
        self.change_at = at;
    }

    pub fn read_and_reset(&mut self, out: &mut [Sample]) {
        let filled_len = self.change_at + 1;
        let copy_len = filled_len.min(out.len());

        out[..copy_len].copy_from_slice(&self.buffer[..copy_len]);

        if out.len() > filled_len {
            out[filled_len..].fill(self.buffer[self.change_at]);
        }

        self.set(self.buffer[self.change_at], 0);
    }
}

pub fn copy_to_display_spectrum(dst: &mut DisplaySpectrum, src: &[ComplexSample]) {
    let len = src.len().min(DISPLAY_SPECTRUM_SIZE);

    dst[..len].copy_from_slice(&src[..len]);
    dst[len..].fill(ComplexSample::ZERO);
}

pub fn copy_to_buffer(buff: &mut [Sample], iter: impl Iterator<Item = Sample>) {
    buff.iter_mut()
        .zip(iter)
        .for_each(|(buff, value)| *buff = value);
}

pub fn add_to_buffer(buff: &mut [Sample], iter: impl Iterator<Item = Sample>) {
    buff.iter_mut()
        .zip(iter)
        .for_each(|(buff, value)| *buff += value);
}

pub fn add_buffer_value(buff: &mut [Sample], value: Sample) {
    buff.iter_mut().for_each(|buff_value| *buff_value += value);
}

pub fn copy_or_add_to_buffer(copy: bool, buff: &mut [Sample], input: impl Iterator<Item = Sample>) {
    if copy {
        copy_to_buffer(buff, input);
    } else {
        add_to_buffer(buff, input);
    }
}
