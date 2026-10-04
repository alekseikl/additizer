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

    // pub fn advance_normalized(&mut self, norm: Sample) {
    //     *self += Self::from_normalized(norm);
    // }
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

/// Four independent [`Phase`]s processed as one SIMD vector.
///
/// Mirrors the scalar [`Phase`] API. The only semantic difference is in the
/// `f32 -> fixed-point` conversions: SIMD lacks a wrapping `f32 -> u32`
/// conversion, so [`PhaseX4::from_normalized`] wraps its input into
/// `[-0.5, 0.5]` first (exact modulo one cycle). [`PhaseX4::from_wrapped`]
/// skips that wrap and requires each lane to already be in that range.
/// [`Add<f32x4>`] saturates instead of wrapping for increments of half a
/// cycle or more per step (i.e. frequencies at or above Nyquist, which are
/// not meaningful anyway).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PhaseX4(u32x4);

impl PhaseX4 {
    pub const ZERO: Self = Self(u32x4::ZERO);

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

    // Splatted constants must be `const` so they become literal vector data.
    // A runtime `splat(constant)` is an `[x; 4]` fill, which LLVM may lower to
    // a `memset_pattern16` call on macOS, which is disastrous in a hot loop.
    const FULL_PHASE: f32x4 = f32x4::splat(Phase::FULL_PHASE);

    /// Wraps each lane into `[-0.5, 0.5]` (exact modulo one cycle).
    #[inline(always)]
    pub fn wrap_normalized(phase: f32x4) -> f32x4 {
        phase - phase.round()
    }

    /// Lane-wise [`Phase::from_normalized`]; see the type docs for rounding
    /// differences.
    #[inline(always)]
    pub fn from_normalized(phase: f32x4) -> Self {
        Self::from_wrapped(Self::wrap_normalized(phase))
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

    /// Lane-wise [`Phase::wave_index_fraction`].
    #[inline(always)]
    pub fn wave_index_fraction<const WAVEFORM_BITS: usize>(self) -> f32x4 {
        let mask = const { u32x4::splat(Phase::intermediate_mask::<WAVEFORM_BITS>()) };
        let mult = const { f32x4::splat(Phase::intermediate_mult::<WAVEFORM_BITS>()) };
        let masked: i32x4 = cast(self.0 & mask);

        masked.round_float() * mult
    }

    /// Lane-wise [`Phase::wave_index`] for a wavetable size chosen at runtime.
    #[inline(always)]
    pub fn wave_index_with(self, scale: WaveIndexScale) -> [u32; 4] {
        (self.0 >> scale.shift).to_array()
    }

    /// Lane-wise [`Phase::wave_index_fraction`] for a wavetable size chosen at runtime.
    #[inline(always)]
    pub fn wave_index_fraction_with(self, scale: WaveIndexScale) -> f32x4 {
        let masked: i32x4 = cast(self.0 & scale.mask);

        masked.round_float() * scale.mult
    }
}

/// Precomputed shift, mask, and fraction scale for a power-of-two wavetable.
///
/// Built once per render so the sample loop does not splat these constants.
#[derive(Clone, Copy)]
pub struct WaveIndexScale {
    shift: u32,
    mask: u32x4,
    mult: f32x4,
}

impl WaveIndexScale {
    #[inline(always)]
    pub fn from_bits(waveform_bits: u32) -> Self {
        let shift = 32 - waveform_bits;

        Self {
            shift,
            mask: u32x4::splat((1u32 << shift) - 1),
            mult: f32x4::splat(((1u32 << shift) as f32).recip()),
        }
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

    /// Advances each lane by a fixed-point increment; see the type docs.
    #[inline(always)]
    fn add(self, rhs: f32x4) -> Self::Output {
        self + Self(cast(rhs.trunc_int()))
    }
}
