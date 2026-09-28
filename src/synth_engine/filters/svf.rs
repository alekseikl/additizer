//! Time-domain state-variable filter in the Cytomic / Andrew Simper form

use std::f32::consts::PI;

use enum_dispatch::enum_dispatch;
use itertools::izip;
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::Sample,
    utils::{C4_PITCH, pitch_to_freq},
};

/// Largest `f / sample_rate` fed to `tan`, keeping `g` finite just below Nyquist.
const MAX_FREQ_RATIO: Sample = 0.499;

#[cfg(test)]
mod tests;

mod response;
mod section;

pub use response::SvfResponse;

use section::{
    BandPass, HighPass, HighShelf, LowPass, LowShelf, OnePoleHighPass, OnePoleLowPass, ShelfCoeffs,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SvfType {
    #[default]
    LowPass12,
    LowPass18,
    LowPass24,
    HighPass12,
    HighPass18,
    HighPass24,
    BandPass6,
    BandPass12,
    Peaking,
    Notch,
    LowShelf12,
    LowShelf24,
    HighShelf12,
    HighShelf24,
}

/// Coefficients and input for one sample.
pub(crate) struct TickParams {
    g: Sample,
    /// `1/Q` of the resonant stage.
    k: Sample,
    /// `1/Q` of the pre stage.
    pre_k: Sample,
    gain: Sample,
    input: Sample,
}

#[enum_dispatch]
pub(crate) trait SvfFilter {
    fn tick(&mut self, p: &TickParams) -> Sample;

    #[allow(clippy::too_many_arguments)]
    fn process(
        &mut self,
        sample_rate: Sample,
        input: &[Sample],
        cutoff: &[Sample],
        k: &[Sample],     // k = 1/Q of the resonant stage
        pre_k: &[Sample], // k = 1/Q of the pre stage
        gain: &[Sample],
        output: &mut [Sample],
    ) {
        let freq_mult = sample_rate.recip();

        for (out, &sample, &cutoff, &k, &pre_k, &gain) in
            izip!(output, input, cutoff, k, pre_k, gain)
        {
            let freq = pitch_to_freq(C4_PITCH + cutoff);

            *out = self.tick(&TickParams {
                g: ((freq * freq_mult).min(MAX_FREQ_RATIO) * PI).tan(),
                k,
                pre_k,
                gain,
                input: sample,
            });
        }
    }
}

#[derive(Default, Clone, Copy)]
pub(crate) struct LowPass12 {
    resonant: LowPass,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct LowPass18 {
    resonant: LowPass,
    one_pole: OnePoleLowPass,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct LowPass24 {
    pre_stage: LowPass,
    resonant: LowPass,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct HighPass12 {
    resonant: HighPass,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct HighPass18 {
    resonant: HighPass,
    one_pole: OnePoleHighPass,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct HighPass24 {
    pre_stage: HighPass,
    resonant: HighPass,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct BandPass6 {
    resonant: BandPass,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct BandPass12 {
    pre_stage: BandPass,
    resonant: BandPass,
}

/// Bell. Linear `gain` is the level at the cutoff; DC and high frequencies stay at unity.
#[derive(Default, Clone, Copy)]
pub(crate) struct Peaking {
    section: section::Peaking,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct Notch {
    resonant: section::Notch,
}

/// Low shelf, 12 dB/oct. Linear `gain` is the DC level; high frequencies stay at unity.
#[derive(Default, Clone, Copy)]
pub(crate) struct LowShelf12 {
    section: LowShelf,
}

/// High shelf, 12 dB/oct. Linear `gain` is the high-frequency level; DC stays at unity.
#[derive(Default, Clone, Copy)]
pub(crate) struct HighShelf12 {
    section: HighShelf,
}

/// Low shelf, 24 dB/oct. Two 12 dB sections, each at `sqrt(gain)`. One stage uses `pre_k`.
#[derive(Default, Clone, Copy)]
pub(crate) struct LowShelf24 {
    pre_stage: LowShelf,
    resonant: LowShelf,
}

/// High shelf, 24 dB/oct. Two 12 dB sections, each at `sqrt(gain)`. One stage uses `pre_k`.
#[derive(Default, Clone, Copy)]
pub(crate) struct HighShelf24 {
    pre_stage: HighShelf,
    resonant: HighShelf,
}

impl SvfFilter for LowPass12 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }
}

impl SvfFilter for LowPass18 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        let x = self.resonant.tick(p.g, p.k, p.input * p.gain);

        self.one_pole.tick(p.g, x)
    }
}

impl SvfFilter for LowPass24 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        let x = self.pre_stage.tick(p.g, p.pre_k, p.input * p.gain);

        self.resonant.tick(p.g, p.k, x)
    }
}

impl SvfFilter for HighPass12 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }
}

impl SvfFilter for HighPass18 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        let x = self.resonant.tick(p.g, p.k, p.input * p.gain);

        self.one_pole.tick(p.g, x)
    }
}

impl SvfFilter for HighPass24 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        let x = self.pre_stage.tick(p.g, p.pre_k, p.input * p.gain);

        self.resonant.tick(p.g, p.k, x)
    }
}

impl SvfFilter for BandPass6 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }
}

impl SvfFilter for BandPass12 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        let x = self.pre_stage.tick(p.g, p.pre_k, p.input * p.gain);

        self.resonant.tick(p.g, p.k, x)
    }
}

impl SvfFilter for Peaking {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        self.section.tick(p.g, p.k, p.gain, p.input)
    }
}

impl SvfFilter for Notch {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }
}

impl SvfFilter for LowShelf12 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        self.section
            .tick(p.g, p.k, ShelfCoeffs::new(p.gain), p.input)
    }
}

impl SvfFilter for HighShelf12 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        self.section
            .tick(p.g, p.k, ShelfCoeffs::new(p.gain), p.input)
    }
}

impl SvfFilter for LowShelf24 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        let coeffs = ShelfCoeffs::cascaded(p.gain);
        let x = self.pre_stage.tick(p.g, p.pre_k, coeffs, p.input);

        self.resonant.tick(p.g, p.k, coeffs, x)
    }
}

impl SvfFilter for HighShelf24 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        let coeffs = ShelfCoeffs::cascaded(p.gain);
        let x = self.pre_stage.tick(p.g, p.pre_k, coeffs, p.input);

        self.resonant.tick(p.g, p.k, coeffs, x)
    }
}

#[enum_dispatch(SvfFilter)]
#[derive(Clone, Copy)]
pub(crate) enum SvfState {
    LowPass12(LowPass12),
    LowPass18(LowPass18),
    LowPass24(LowPass24),
    HighPass12(HighPass12),
    HighPass18(HighPass18),
    HighPass24(HighPass24),
    BandPass6(BandPass6),
    BandPass12(BandPass12),
    Peaking(Peaking),
    Notch(Notch),
    LowShelf12(LowShelf12),
    LowShelf24(LowShelf24),
    HighShelf12(HighShelf12),
    HighShelf24(HighShelf24),
}

impl Default for SvfState {
    fn default() -> Self {
        Self::LowPass12(LowPass12::default())
    }
}

impl SvfState {
    pub fn new(filter_type: SvfType) -> Self {
        match filter_type {
            SvfType::LowPass12 => Self::LowPass12(LowPass12::default()),
            SvfType::LowPass18 => Self::LowPass18(LowPass18::default()),
            SvfType::LowPass24 => Self::LowPass24(LowPass24::default()),
            SvfType::HighPass12 => Self::HighPass12(HighPass12::default()),
            SvfType::HighPass18 => Self::HighPass18(HighPass18::default()),
            SvfType::HighPass24 => Self::HighPass24(HighPass24::default()),
            SvfType::BandPass6 => Self::BandPass6(BandPass6::default()),
            SvfType::BandPass12 => Self::BandPass12(BandPass12::default()),
            SvfType::Peaking => Self::Peaking(Peaking::default()),
            SvfType::Notch => Self::Notch(Notch::default()),
            SvfType::LowShelf12 => Self::LowShelf12(LowShelf12::default()),
            SvfType::LowShelf24 => Self::LowShelf24(LowShelf24::default()),
            SvfType::HighShelf12 => Self::HighShelf12(HighShelf12::default()),
            SvfType::HighShelf24 => Self::HighShelf24(HighShelf24::default()),
        }
    }

    fn current_type(&self) -> SvfType {
        match self {
            Self::LowPass12(_) => SvfType::LowPass12,
            Self::LowPass18(_) => SvfType::LowPass18,
            Self::LowPass24(_) => SvfType::LowPass24,
            Self::HighPass12(_) => SvfType::HighPass12,
            Self::HighPass18(_) => SvfType::HighPass18,
            Self::HighPass24(_) => SvfType::HighPass24,
            Self::BandPass6(_) => SvfType::BandPass6,
            Self::BandPass12(_) => SvfType::BandPass12,
            Self::Peaking(_) => SvfType::Peaking,
            Self::Notch(_) => SvfType::Notch,
            Self::LowShelf12(_) => SvfType::LowShelf12,
            Self::LowShelf24(_) => SvfType::LowShelf24,
            Self::HighShelf12(_) => SvfType::HighShelf12,
            Self::HighShelf24(_) => SvfType::HighShelf24,
        }
    }

    /// Replaces the variant when `filter_type` changes, which clears integrator state.
    pub(crate) fn set_type(&mut self, filter_type: SvfType) {
        if self.current_type() != filter_type {
            *self = Self::new(filter_type);
        }
    }
}
