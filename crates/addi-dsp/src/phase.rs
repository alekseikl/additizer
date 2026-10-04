use std::ops::{Add, AddAssign};

use serde::{Deserialize, Serialize};
use wide::{bytemuck::cast, f32x4, i32x4, u32x4};

use crate::Sample;

#[cfg(test)]
mod tests;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Phase(u32);

impl Phase {
    pub const ZERO: Self = Self(0);

    const FULL_PHASE: Sample = ((u32::MAX as u64) + 1) as Sample;

    #[inline(always)]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

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

impl AddAssign<Sample> for Phase {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Sample) {
        *self = *self + rhs;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PhaseX4(u32x4);

impl PhaseX4 {
    #[inline(always)]
    pub fn load(phases: &[Phase; 4]) -> Self {
        Self(u32x4::new(phases.map(Phase::bits)))
    }

    #[inline(always)]
    pub fn store(self, phases: &mut [Phase; 4]) {
        *phases = self.0.to_array().map(Phase::from_bits);
    }

    #[inline(always)]
    pub fn splat(phase: Phase) -> Self {
        Self(u32x4::splat(phase.bits()))
    }

    const FULL_PHASE: f32x4 = f32x4::splat(Phase::FULL_PHASE);

    /// Wraps each lane into `[-0.5, 0.5]` (exact modulo one cycle).
    #[inline(always)]
    pub fn wrap_normalized(phase: f32x4) -> f32x4 {
        phase - phase.round()
    }

    /// Lane-wise [`Phase::from_normalized`] for a phase already in `[-0.5, 0.5]`.
    ///
    /// `+0.5` does not fit in `i32` after scaling by `2^32` and saturates one
    /// LSB short of half a cycle.
    #[inline(always)]
    pub fn from_wrapped(phase: f32x4) -> Self {
        Self(cast((phase * Self::FULL_PHASE).trunc_int()))
    }

    /// Lane-wise [`Phase::wave_index`].
    #[inline(always)]
    pub fn wave_index<const WAVEFORM_BITS: usize>(self) -> [u32; 4] {
        (self.0 >> Phase::intermediate_bits::<WAVEFORM_BITS>() as u32).to_array()
    }

    #[inline(always)]
    pub fn wave_index_fraction<const WAVEFORM_BITS: usize>(self) -> f32x4 {
        let mask = const { u32x4::splat(Phase::intermediate_mask::<WAVEFORM_BITS>()) };
        let mult = const { f32x4::splat(Phase::intermediate_mult::<WAVEFORM_BITS>()) };
        let masked: i32x4 = cast(self.0 & mask);

        masked.round_float() * mult
    }
}

impl Add for PhaseX4 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<f32x4> for PhaseX4 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f32x4) -> Self::Output {
        self + Self(cast(rhs.trunc_int()))
    }
}
