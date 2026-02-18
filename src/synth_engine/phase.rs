use std::ops::{Add, AddAssign};

use serde::{Deserialize, Serialize};

use crate::synth_engine::Sample;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phase(u32);

impl Phase {
    pub const ZERO: Self = Self(0);

    const FULL_PHASE: Sample = ((u32::MAX as u64) + 1) as Sample;

    #[inline(always)]
    pub const fn freq_phase_mult(sample_rate: Sample) -> Sample {
        Self::FULL_PHASE / sample_rate
    }

    #[inline(always)]
    pub fn from_normalized(phase: Sample) -> Self {
        Self((phase * Self::FULL_PHASE) as i64 as u32)
    }

    #[inline(always)]
    pub fn value(&self) -> u32 {
        self.0
    }

    pub fn normalized(&self) -> Sample {
        self.0 as Sample / Self::FULL_PHASE
    }

    pub fn add_normalized(self, norm: Sample) -> Self {
        self + Self::from_normalized(norm)
    }

    pub fn advance_normalized(&mut self, norm: Sample) {
        *self += Self::from_normalized(norm);
    }
}

impl Add for Phase {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl Add<Sample> for Phase {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Sample) -> Self::Output {
        self + Self(rhs as i64 as u32)
    }
}

impl AddAssign for Phase {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl AddAssign<Sample> for Phase {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Sample) {
        *self = *self + rhs;
    }
}
