use realfft::num_complex::Complex;
use serde::{Deserialize, Serialize};

pub type Sample = f32;
pub type Phase = u32;
pub type ComplexSample = Complex<Sample>;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct StereoValue {
    pub left: Sample,
    pub right: Sample,
}

impl StereoValue {
    pub fn new(l: Sample, r: Sample) -> Self {
        Self { left: l, right: r }
    }

    pub fn mono(lr: Sample) -> Self {
        Self {
            left: lr,
            right: lr,
        }
    }

    pub fn iter(&self) -> StereoValueIter<'_> {
        StereoValueIter {
            value: self,
            idx: 0,
        }
    }
}

impl From<f32> for StereoValue {
    fn from(value: f32) -> Self {
        Self::mono(value)
    }
}

pub struct StereoValueIter<'a> {
    value: &'a StereoValue,
    idx: usize,
}

impl Iterator for StereoValueIter<'_> {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let value = match self.idx {
            0 => Some(self.value.left),
            1 => Some(self.value.right),
            _ => None,
        };

        self.idx += 1;
        value
    }
}
