use std::ops::{Add, AddAssign, Mul};

pub struct StereoSample(pub f32, pub f32);

impl StereoSample {
    pub fn iter(&self) -> StereoSampleIterator {
        StereoSampleIterator {
            sample: self,
            idx: 0,
        }
    }
}

impl Add for StereoSample {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        StereoSample(self.0 + rhs.0, self.1 + rhs.1)
    }
}

impl AddAssign for StereoSample {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
        self.1 += rhs.1;
    }
}

impl Mul<f32> for StereoSample {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs, self.1 * rhs)
    }
}

pub struct StereoSampleIterator<'a> {
    sample: &'a StereoSample,
    idx: usize,
}

impl Iterator for StereoSampleIterator<'_> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let value = match self.idx {
            0 => Some(self.sample.0),
            1 => Some(self.sample.1),
            _ => None,
        };
        self.idx += 1;

        value
    }
}
