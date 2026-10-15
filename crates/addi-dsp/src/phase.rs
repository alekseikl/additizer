use std::ops::{Add, AddAssign};

use serde::{Deserialize, Serialize};
use wide::bytemuck::cast;

use crate::{SIMD_LANES, Smp, WideI32, WideSmp, WideU32};

#[cfg(test)]
mod tests;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Phase(u32);

impl Phase {
    pub const ZERO: Self = Self(0);

    const FULL_PHASE: Smp = ((u32::MAX as u64) + 1) as Smp;

    #[inline(always)]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    #[inline(always)]
    pub const fn freq_phase_mult(sample_rate: Smp) -> Smp {
        Self::FULL_PHASE / sample_rate
    }

    /// Hz from a phase increment built with [`Self::freq_phase_mult`].
    #[inline(always)]
    pub fn phase_inc_to_freq(phase_inc: Smp, freq_phase_mult: Smp) -> Smp {
        phase_inc / freq_phase_mult
    }

    #[inline(always)]
    pub fn from_normalized(phase: Smp) -> Self {
        Self((phase * Self::FULL_PHASE) as i64 as u32)
    }

    const fn intermediate_bits<const WAVEFORM_BITS: usize>() -> usize {
        32 - WAVEFORM_BITS
    }

    const fn intermediate_mask<const WAVEFORM_BITS: usize>() -> u32 {
        (1 << Self::intermediate_bits::<WAVEFORM_BITS>()) - 1
    }

    const fn intermediate_mult<const WAVEFORM_BITS: usize>() -> Smp {
        ((1 << Self::intermediate_bits::<WAVEFORM_BITS>()) as Smp).recip()
    }

    #[inline(always)]
    pub fn wave_index<const WAVEFORM_BITS: usize>(&self) -> usize {
        (self.0 >> Self::intermediate_bits::<WAVEFORM_BITS>()) as usize
    }

    #[inline(always)]
    pub fn wave_index_fraction<const WAVEFORM_BITS: usize>(&self) -> Smp {
        (self.0 & Self::intermediate_mask::<WAVEFORM_BITS>()) as Smp
            * Self::intermediate_mult::<WAVEFORM_BITS>()
    }

    pub fn normalized(&self) -> Smp {
        self.0 as Smp / Self::FULL_PHASE
    }

    pub fn add_normalized(self, norm: Smp) -> Self {
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

impl Add<Smp> for Phase {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Smp) -> Self::Output {
        self + Self(rhs as i64 as u32)
    }
}

impl AddAssign<Smp> for Phase {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Smp) {
        *self = *self + rhs;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WidePhase(WideU32);

impl WidePhase {
    #[inline(always)]
    pub fn load(phases: &[Phase; SIMD_LANES]) -> Self {
        Self(WideU32::new(phases.map(Phase::bits)))
    }

    #[inline(always)]
    pub fn store(self, phases: &mut [Phase; SIMD_LANES]) {
        *phases = self.0.to_array().map(Phase::from_bits);
    }

    #[inline(always)]
    pub fn splat(phase: Phase) -> Self {
        Self(WideU32::splat(phase.bits()))
    }

    const FULL_PHASE: WideSmp = WideSmp::splat(Phase::FULL_PHASE);

    /// Lane-wise [`Phase::from_normalized`].
    ///
    /// Wraps into `[-0.5, 0.5]` first: `+0.5` does not fit in `i32` after
    /// scaling by `2^32` and would saturate one LSB short of half a cycle.
    #[inline(always)]
    pub fn from_normalized(phase: WideSmp) -> Self {
        let wrapped = phase - phase.round();
        Self(cast((wrapped * Self::FULL_PHASE).trunc_int()))
    }

    /// Lane-wise [`Phase::wave_index`].
    #[inline(always)]
    pub fn wave_index<const WAVEFORM_BITS: usize>(self) -> [u32; SIMD_LANES] {
        (self.0 >> Phase::intermediate_bits::<WAVEFORM_BITS>() as u32).to_array()
    }

    #[inline(always)]
    pub fn wave_index_fraction<const WAVEFORM_BITS: usize>(self) -> WideSmp {
        let mask = const { WideU32::splat(Phase::intermediate_mask::<WAVEFORM_BITS>()) };
        let mult = const { WideSmp::splat(Phase::intermediate_mult::<WAVEFORM_BITS>()) };
        let masked: WideI32 = cast(self.0 & mask);

        masked.round_float() * mult
    }
}

impl Add for WidePhase {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<WideSmp> for WidePhase {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: WideSmp) -> Self::Output {
        self + Self(cast(rhs.trunc_int()))
    }
}
