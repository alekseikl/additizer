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

use section::{BandPass, HighPass, HighShelf, LowPass, LowShelf, OnePoleHighPass, OnePoleLowPass};

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
    HighShelf12,
}

impl SvfType {
    pub const ALL: [Self; 12] = [
        Self::LowPass12,
        Self::LowPass18,
        Self::LowPass24,
        Self::HighPass12,
        Self::HighPass18,
        Self::HighPass24,
        Self::BandPass6,
        Self::BandPass12,
        Self::Peaking,
        Self::Notch,
        Self::LowShelf12,
        Self::HighShelf12,
    ];
}

#[enum_dispatch]
pub(crate) trait SvfFilter {
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample;
    fn reset(&mut self);

    fn process(
        &mut self,
        sample_rate: Sample,
        input: &[Sample],
        cutoff: &[Sample],
        q: &[Sample],
        gain: &[Sample],
        output: &mut [Sample],
    ) {
        let g_table = g_table();
        let freq_mult = (sample_rate * RANGE_SCALE).recip();

        for (out, &sample, &cutoff, &q, &gain) in izip!(output, input, cutoff, q, gain) {
            let freq = pitch_to_freq(C4_PITCH + cutoff);

            *out = self.tick(
                g_table.at((freq * freq_mult).min(1.0)),
                q.recip(),
                gain,
                sample,
            );
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

impl SvfFilter for LowPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input * gain)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for LowPass18 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.resonant.tick(g, k, input * gain);

        self.one_pole.tick(g, x)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for LowPass24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input * gain);

        self.resonant.tick(g, k, x)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for HighPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input * gain)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for HighPass18 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.resonant.tick(g, k, input * gain);

        self.one_pole.tick(g, x)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for HighPass24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input * gain);

        self.resonant.tick(g, k, x)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for BandPass6 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input * gain)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for BandPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input * gain);

        self.resonant.tick(g, k, x)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for Peaking {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.section.tick(g, k, gain, input)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for Notch {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input * gain)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for LowShelf12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.section.tick(g, k, gain, input)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for HighShelf12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        self.section.tick(g, k, gain, input)
    }

    fn reset(&mut self) {
        *self = Self::default();
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
    HighShelf12(HighShelf12),
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
            SvfType::HighShelf12 => Self::HighShelf12(HighShelf12::default()),
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
            Self::HighShelf12(_) => SvfType::HighShelf12,
        }
    }

    /// Replaces the variant when `filter_type` changes, which clears integrator state.
    pub(crate) fn set_type(&mut self, filter_type: SvfType) {
        if self.current_type() != filter_type {
            *self = Self::new(filter_type);
        }
    }
}
