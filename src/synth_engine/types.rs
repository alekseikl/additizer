use std::ops::Index;

use realfft::num_complex::Complex;
use serde::{Deserialize, Serialize};

use crate::synth_engine::routing::NUM_CHANNELS;

pub type Sample = f32;
pub type Phase = u32;
pub type ComplexSample = Complex<Sample>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StereoSample {
    channels: [Sample; NUM_CHANNELS],
}

impl StereoSample {
    pub fn new(l: Sample, r: Sample) -> Self {
        Self { channels: [l, r] }
    }

    pub fn mono(lr: Sample) -> Self {
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
}

impl Index<usize> for StereoSample {
    type Output = Sample;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.channels[index]
    }
}

impl From<f32> for StereoSample {
    fn from(value: f32) -> Self {
        Self::mono(value)
    }
}
