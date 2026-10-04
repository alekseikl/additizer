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

pub const fn zero_buffer() -> Buffer {
    [0.0; BUFFER_SIZE]
}

pub const fn zero_spectral_buffer() -> SpectralBuffer {
    [ComplexSample::ZERO; SPECTRAL_BUFFER_SIZE]
}

type VoicesLayoutArray<T> = [[T; NUM_CHANNELS]; MAX_VOICES];

pub struct VoicesLayout<T> {
    voices: Box<VoicesLayoutArray<T>>,
}

impl<T> VoicesLayout<T> {
    #[inline]
    pub fn at(&self, channel_idx: usize, voice_idx: usize) -> &T {
        &self.voices[voice_idx][channel_idx]
    }

    #[inline]
    pub fn at_mut(&mut self, channel_idx: usize, voice_idx: usize) -> &mut T {
        &mut self.voices[voice_idx][channel_idx]
    }

    #[inline]
    pub fn channels_at(&self, voice_idx: usize) -> &[T; NUM_CHANNELS] {
        &self.voices[voice_idx]
    }

    #[inline]
    pub fn channels_at_mut(&mut self, voice_idx: usize) -> &mut [T; NUM_CHANNELS] {
        &mut self.voices[voice_idx]
    }

    /// Mutable references to two different slots. Argument order is preserved.
    pub fn at_pair_mut(
        &mut self,
        channel_a: usize,
        voice_a: usize,
        channel_b: usize,
        voice_b: usize,
    ) -> (&mut T, &mut T) {
        assert!(
            channel_a != channel_b || voice_a != voice_b,
            "slots must differ"
        );

        if voice_a == voice_b {
            let channels = self.channels_at_mut(voice_a);

            if channel_a < channel_b {
                let (left, right) = channels.split_at_mut(channel_b);
                (&mut left[channel_a], &mut right[0])
            } else {
                let (left, right) = channels.split_at_mut(channel_a);
                (&mut right[0], &mut left[channel_b])
            }
        } else if voice_a < voice_b {
            let (left, right) = self.voices.split_at_mut(voice_b);
            (&mut left[voice_a][channel_a], &mut right[0][channel_b])
        } else {
            let (left, right) = self.voices.split_at_mut(voice_a);
            (&mut right[0][channel_a], &mut left[voice_b][channel_b])
        }
    }
}

pub type MonoVoicesLayoutArray<T> = [T; MAX_VOICES];
pub type MonoVoicesLayout<T> = Box<MonoVoicesLayoutArray<T>>;

pub fn new_voices_layout<U: Default + Send>() -> VoicesLayout<U> {
    new_voices_layout_with(U::default)
}

pub fn new_voices_layout_with<U: Send>(init: impl FnMut() -> U) -> VoicesLayout<U> {
    let mut voices: Box<[MaybeUninit<[U; NUM_CHANNELS]>; MAX_VOICES]> =
        Box::new([const { MaybeUninit::uninit() }; MAX_VOICES]);
    let mut init = init;

    for voice in voices.iter_mut() {
        init_array_in_place::<U, NUM_CHANNELS>(voice.as_mut_ptr(), &mut init);
    }

    VoicesLayout {
        voices: unsafe { Box::from_raw(Box::into_raw(voices).cast::<VoicesLayoutArray<U>>()) },
    }
}

pub fn new_mono_voices_layout<U: Default + Send>() -> MonoVoicesLayout<U> {
    let mut voices: Box<MaybeUninit<[U; MAX_VOICES]>> = Box::new(MaybeUninit::uninit());
    init_array_in_place::<U, MAX_VOICES>(voices.as_mut_ptr(), &mut U::default);
    unsafe { Box::from_raw(Box::into_raw(voices).cast::<[U; MAX_VOICES]>()) }
}

fn init_array_in_place<U, const N: usize>(dst: *mut [U; N], init: &mut impl FnMut() -> U) {
    let elements = dst.cast::<U>();

    for i in 0..N {
        unsafe {
            elements.add(i).write(init());
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

pub fn copy_to_display_spectrum(dst: &mut [ComplexSample], src: &[ComplexSample]) {
    let len = src.len().min(dst.len());

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

#[cfg(test)]
mod tests;
