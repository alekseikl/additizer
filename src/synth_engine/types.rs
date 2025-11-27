use std::ops::{Add, Div, Index, Mul, Sub};

use realfft::num_complex::Complex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    buffer::{SpectralBuffer, make_zero_spectral_buffer},
    routing::NUM_CHANNELS,
};

pub type Sample = f32;
pub type Phase = u32;
pub type ComplexSample = Complex<Sample>;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StereoSample {
    channels: [Sample; NUM_CHANNELS],
}

impl StereoSample {
    pub const ZERO: StereoSample = StereoSample::splat(0.0);
    pub const ONE: StereoSample = StereoSample::splat(1.0);

    pub const fn new(l: Sample, r: Sample) -> Self {
        Self { channels: [l, r] }
    }

    pub const fn splat(lr: Sample) -> Self {
        Self { channels: [lr, lr] }
    }

    #[inline]
    pub fn left(&self) -> Sample {
        self.channels[0]
    }

    #[inline]
    pub fn right(&self) -> Sample {
        self.channels[1]
    }

    #[inline]
    pub fn set_left(&mut self, left: Sample) {
        self.channels[0] = left;
    }

    #[inline]
    pub fn set_right(&mut self, right: Sample) {
        self.channels[1] = right;
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Sample> {
        self.channels.iter()
    }

    pub fn clamp(&self, min: Sample, max: Sample) -> Self {
        Self {
            channels: self.channels.map(|channel| channel.clamp(min, max)),
        }
    }

    pub fn powf(&self, n: Sample) -> Self {
        Self {
            channels: self.channels.map(|channel| channel.powf(n)),
        }
    }

    pub fn signum(&self) -> Self {
        Self {
            channels: self.channels.map(|channel| channel.signum()),
        }
    }

    pub fn abs(&self) -> Self {
        Self {
            channels: self.channels.map(|channel| channel.abs()),
        }
    }
}

impl Index<usize> for StereoSample {
    type Output = Sample;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.channels[index]
    }
}

macro_rules! stereo_op {
    ($trait:ident, $func:ident, $op:tt) => {
        impl $trait for StereoSample {
            type Output = Self;

            #[allow(clippy::assign_op_pattern)]
            fn $func(mut self, rhs: Self) -> Self::Output {
                for (lhs, rhs) in self.channels.iter_mut().zip(rhs.channels) {
                    *lhs = *lhs $op rhs;
                }
                self
            }
        }

        impl $trait<Sample> for StereoSample {
            type Output = Self;

            #[allow(clippy::assign_op_pattern)]
            fn $func(mut self, rhs: Sample) -> Self::Output {
                for lhs in &mut self.channels {
                    *lhs = *lhs $op rhs;
                }
                self
            }
        }
    };
}

stereo_op! {Add, add, +}
stereo_op! {Sub, sub, -}
stereo_op! {Mul, mul, *}
stereo_op! {Div, div, /}

impl From<f32> for StereoSample {
    fn from(value: f32) -> Self {
        Self::splat(value)
    }
}

impl FromIterator<Sample> for StereoSample {
    fn from_iter<T: IntoIterator<Item = Sample>>(iter: T) -> Self {
        let mut value = Self::splat(0.0);

        for (lhs, rhs) in value.channels.iter_mut().zip(iter) {
            *lhs = rhs;
        }
        value
    }
}

#[derive(Default)]
pub struct ScalarOutput {
    output: [Sample; 2],
}

impl ScalarOutput {
    pub fn current(&self) -> Sample {
        self.output[1]
    }

    pub fn get(&self, current: bool) -> Sample {
        self.output[current as usize]
    }

    pub fn advance(&mut self, next: Sample) {
        self.output[0] = self.output[1];
        self.output[1] = next;
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
            output: [make_zero_spectral_buffer(), make_zero_spectral_buffer()],
        }
    }
}

impl SpectralOutput {
    pub fn get(&self, current: bool) -> &SpectralBuffer {
        &self.output[(current ^ self.swapped) as usize]
    }

    pub fn advance(&mut self) -> &mut SpectralBuffer {
        self.swapped = !self.swapped;
        &mut self.output[!self.swapped as usize]
    }
}
