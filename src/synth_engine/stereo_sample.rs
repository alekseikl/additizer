use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Index, IndexMut, Mul, Sub};

use crate::synth_engine::{Sample, routing::NUM_CHANNELS};

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

    #[inline]
    pub fn map(&self, f: impl FnMut(Sample) -> Sample) -> Self {
        Self {
            channels: self.channels.map(f),
        }
    }

    pub fn clamp(&self, min: Sample, max: Sample) -> Self {
        self.map(|channel| channel.clamp(min, max))
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

impl IndexMut<usize> for StereoSample {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.channels[index]
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
