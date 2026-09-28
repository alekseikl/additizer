//! Time-domain state-variable filter in the Cytomic / Andrew Simper form

use std::{f32::consts::PI, sync::LazyLock};

use enum_dispatch::enum_dispatch;
use itertools::izip;
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        Sample,
        lookup_table::{EXTRA_SAMPLES, LookupTable},
    },
    utils::{C4_PITCH, pitch_to_freq},
};

const G_TABLE_INTERVALS: usize = 2048;
const RANGE_SCALE: Sample = 0.5;

/// `G_TABLE_INTERVALS` steps of `f / sample_rate` on `[0, 1/2]`, clamped at `0.499` before `tan`.
type GTable = LookupTable<{ G_TABLE_INTERVALS + EXTRA_SAMPLES }>;

fn g_table() -> &'static GTable {
    const MAX_RATIO: Sample = 0.499;

    static TABLE: LazyLock<GTable> =
        LazyLock::new(|| LookupTable::new(|t| ((RANGE_SCALE * t).min(MAX_RATIO) * PI).tan()));

    &TABLE
}

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

#[enum_dispatch]
pub(crate) trait SvfFilter {
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample;

    fn process(
        &mut self,
        sample_rate: Sample,
        input: &[Sample],
        cutoff: &[Sample],
        k: &[Sample], // k = 1/Q
        gain: &[Sample],
        output: &mut [Sample],
    ) {
        let g_table = g_table();
        let freq_mult = (sample_rate * RANGE_SCALE).recip();

        for (out, &sample, &cutoff, &k, &gain) in izip!(output, input, cutoff, k, gain) {
            let freq = pitch_to_freq(C4_PITCH + cutoff);
            let g = g_table.at((freq * freq_mult).min(1.0));

            *out = self.tick(g, k, gain, sample);
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
    fixed: LowPass,
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
    fixed: HighPass,
    resonant: HighPass,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct BandPass6 {
    resonant: BandPass,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct BandPass12 {
    fixed: BandPass,
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

/// Low shelf, 24 dB/oct. Two 12 dB sections, each at `sqrt(gain)`, one fixed at Q = 1.
#[derive(Default, Clone, Copy)]
pub(crate) struct LowShelf24 {
    fixed: LowShelf,
    resonant: LowShelf,
}

/// High shelf, 24 dB/oct. Two 12 dB sections, each at `sqrt(gain)`, one fixed at Q = 1.
#[derive(Default, Clone, Copy)]
pub(crate) struct HighShelf24 {
    fixed: HighShelf,
    resonant: HighShelf,
}

impl SvfFilter for LowPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input * gain)
    }
}

impl SvfFilter for LowPass18 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.resonant.tick(g, k, input * gain);

        self.one_pole.tick(g, x)
    }
}

impl SvfFilter for LowPass24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input * gain);

        self.resonant.tick(g, k, x)
    }
}

impl SvfFilter for HighPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input * gain)
    }
}

impl SvfFilter for HighPass18 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.resonant.tick(g, k, input * gain);

        self.one_pole.tick(g, x)
    }
}

impl SvfFilter for HighPass24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input * gain);

        self.resonant.tick(g, k, x)
    }
}

impl SvfFilter for BandPass6 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input * gain)
    }
}

impl SvfFilter for BandPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input * gain);

        self.resonant.tick(g, k, x)
    }
}

impl SvfFilter for Peaking {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.section.tick(g, k, gain, input)
    }
}

impl SvfFilter for Notch {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input * gain)
    }
}

impl SvfFilter for LowShelf12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.section.tick(g, k, ShelfCoeffs::new(gain), input)
    }
}

impl SvfFilter for HighShelf12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.section.tick(g, k, ShelfCoeffs::new(gain), input)
    }
}

impl SvfFilter for LowShelf24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let coeffs = ShelfCoeffs::cascaded(gain);
        let x = self.fixed.tick(g, 1.0, coeffs, input);

        self.resonant.tick(g, k, coeffs, x)
    }
}

impl SvfFilter for HighShelf24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let coeffs = ShelfCoeffs::cascaded(gain);
        let x = self.fixed.tick(g, 1.0, coeffs, input);

        self.resonant.tick(g, k, coeffs, x)
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
