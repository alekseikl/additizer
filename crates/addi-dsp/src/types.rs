use num_complex::Complex;
use wide::f32x4;

pub type Smp = f32;
pub type WideSmp = f32x4;
pub type ComplexSample = Complex<Smp>;

/// Width of a [`WideSmp`] vector.
pub const SIMD_LANES: usize = 4;
