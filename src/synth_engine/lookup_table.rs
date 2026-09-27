//! Uniform lookup table with Catmull-Rom interpolation.

use wide::f32x4;

use crate::synth_engine::{Sample, coeffs::catmull_rom};

const PAD_LEFT: usize = 1;
const PAD_RIGHT: usize = 2;

/// Samples stored beyond the interval count: one before `t = 0`, the sample at
/// `t = 1`, and two past `t = 1`.
pub const EXTRA_SAMPLES: usize = PAD_LEFT + PAD_RIGHT + 1;

/// Catmull-Rom table of `N` samples.
///
/// `N` includes one sample in front at `t = -step` and two past the end at
/// `t = 1 + step` and `t = 1 + 2 * step`, so `f` must be defined there. The samples
/// between them divide `t` in `[0, 1]` into `intervals = N - EXTRA_SAMPLES` equal steps.
pub struct LookupTable<const N: usize> {
    samples: [Sample; N],
}

impl<const N: usize> LookupTable<N> {
    const INTERVALS: usize = N - EXTRA_SAMPLES;

    pub fn new(f: impl Fn(Sample) -> Sample) -> Self {
        const {
            assert!(
                Self::INTERVALS >= 1,
                "lookup table needs at least one interval"
            );
        };

        let step = (Self::INTERVALS as Sample).recip();
        let samples = std::array::from_fn(|i| f((i as Sample - PAD_LEFT as Sample) * step));

        Self { samples }
    }

    #[inline(always)]
    pub fn at(&self, t: Sample) -> Sample {
        const {
            assert!(
                Self::INTERVALS >= 1,
                "lookup table needs at least one interval"
            );
        };

        let x = t * Self::INTERVALS as Sample;
        let idx = (x as usize).min(Self::INTERVALS);
        let frac = x - idx as Sample;
        let s = &self.samples[idx..idx + 4];

        Self::interpolate(f32x4::new([s[0], s[1], s[2], s[3]]), frac)
    }

    #[inline(always)]
    fn interpolate(c: f32x4, t: Sample) -> Sample {
        let poly = catmull_rom(t);

        (poly * c).reduce_add()
    }
}

#[cfg(test)]
mod tests;
