use std::ops::{Div, Mul};

use crate::WideSmp;

/// Four complex values, real and imag in separate lanes.
#[derive(Clone, Copy)]
pub(super) struct ComplexX4 {
    pub(super) re: WideSmp,
    pub(super) im: WideSmp,
}

impl ComplexX4 {
    #[inline(always)]
    pub(super) fn new(re: WideSmp, im: WideSmp) -> Self {
        Self { re, im }
    }

    #[inline(always)]
    pub(super) fn from_real(re: WideSmp) -> Self {
        Self {
            re,
            im: WideSmp::ZERO,
        }
    }

    /// `|z|` as `sqrt(re² + im²)`. Matches `f32` hypot for filter-sized values.
    #[inline(always)]
    pub(super) fn norm(self) -> WideSmp {
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

impl Mul<WideSmp> for ComplexX4 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: WideSmp) -> Self {
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
