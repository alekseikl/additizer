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

    const fn intermediate_bits<const WAVEFORM_BITS: usize>() -> usize {
        32 - WAVEFORM_BITS
    }

    const fn intermediate_mask<const WAVEFORM_BITS: usize>() -> u32 {
        (1 << Self::intermediate_bits::<WAVEFORM_BITS>()) - 1
    }

    const fn intermediate_mult<const WAVEFORM_BITS: usize>() -> Sample {
        ((1 << Self::intermediate_bits::<WAVEFORM_BITS>()) as Sample).recip()
    }

    #[inline(always)]
    pub fn wave_index<const WAVEFORM_BITS: usize>(&self) -> usize {
        (self.0 >> Self::intermediate_bits::<WAVEFORM_BITS>()) as usize
    }

    #[inline(always)]
    pub fn wave_index_fraction<const WAVEFORM_BITS: usize>(&self) -> Sample {
        (self.0 & Self::intermediate_mask::<WAVEFORM_BITS>()) as Sample
            * Self::intermediate_mult::<WAVEFORM_BITS>()
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
