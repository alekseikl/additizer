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

pub type Buffer = [Sample; BUFFER_SIZE];
pub type SpectralBuffer = [ComplexSample; SPECTRAL_BUFFER_SIZE];

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

/// Stereo channel layout `[[U; N]; NUM_CHANNELS]`, heap-allocated.
///
/// Each inner `U` is [`Default`]-initialized in place so callers never materialize
/// a full `[U; N]` (potentially hundreds of KB) on the stack.
pub fn new_voices_layout<U: Default + Send>() -> VoicesLayout<U> {
    let mut channels: Box<[MaybeUninit<[U; MAX_VOICES]>; NUM_CHANNELS]> =
        Box::new([const { MaybeUninit::uninit() }; NUM_CHANNELS]);

    for channel in channels.iter_mut() {
        init_array_in_place::<U, MAX_VOICES>(channel.as_mut_ptr());
    }

    unsafe { Box::from_raw(Box::into_raw(channels).cast::<[[U; MAX_VOICES]; NUM_CHANNELS]>()) }
}

fn init_array_in_place<U: Default, const N: usize>(dst: *mut [U; N]) {
    let elements = dst.cast::<U>();

    for i in 0..N {
        unsafe {
            elements.add(i).write(U::default());
        }
    }
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
