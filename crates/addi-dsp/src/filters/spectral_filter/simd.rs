use std::ops::{Div, Mul};

use wide::f32x4;

/// Four complex values, real and imag in separate lanes.
#[derive(Clone, Copy)]
pub(super) struct ComplexX4 {
    pub(super) re: f32x4,
    pub(super) im: f32x4,
}

impl ComplexX4 {
    #[inline(always)]
    pub(super) fn new(re: f32x4, im: f32x4) -> Self {
        Self { re, im }
    }

    #[inline(always)]
    pub(super) fn from_real(re: f32x4) -> Self {
        Self {
            re,
            im: f32x4::ZERO,
        }
    }

    /// `|z|` as `sqrt(re² + im²)`. Matches `f32` hypot for filter-sized values.
    #[inline(always)]
    pub(super) fn norm(self) -> f32x4 {
        self.re.mul_add(self.re, self.im * self.im).sqrt()
    }
}

impl Mul for ComplexX4 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re.mul_add(rhs.re, -(self.im * rhs.im)),
            im: self.re.mul_add(rhs.im, self.im * rhs.re),
        }
    }
}

impl Mul<f32x4> for ComplexX4 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: f32x4) -> Self {
        Self {
            re: self.re * rhs,
            im: self.im * rhs,
        }
    }
}

impl Div for ComplexX4 {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self {
        let den = rhs.re.mul_add(rhs.re, rhs.im * rhs.im);
        Self {
            re: self.re.mul_add(rhs.re, self.im * rhs.im) / den,
            im: self.im.mul_add(rhs.re, -(self.re * rhs.im)) / den,
        }
    }
}
