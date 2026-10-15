use num_complex::Complex;
use wide::{f32x4, i32x4, u32x4};

pub type Smp = f32;
pub type WideSmp = f32x4;
pub type WideU32 = u32x4;
pub type WideI32 = i32x4;
pub type ComplexSample = Complex<Smp>;

/// Width of a [`WideSmp`] vector.
pub const SIMD_LANES: usize = 4;
