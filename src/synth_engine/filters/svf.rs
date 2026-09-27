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

pub const MIN_RESONANCE: Sample = 0.0;
pub const MAX_RESONANCE: Sample = 1.0;

const ZERO_RESONANCE_Q: Sample = 0.5;
const MAX_RESONANCE_Q: Sample = 16.0;

/// Clamp for `f / sample_rate` before `tan(π t)` (`ω = 0.499 π`).
const MAX_FREQ_RATIO: Sample = 0.499;

const G_TABLE_INTERVALS: usize = 2048;

/// `G_TABLE_INTERVALS` steps on `[0, 1]`, plus the Catmull-Rom pads.
type GTable = LookupTable<{ G_TABLE_INTERVALS + EXTRA_SAMPLES }>;

fn g_table() -> &'static GTable {
    static TABLE: LazyLock<GTable> =
        LazyLock::new(|| LookupTable::new(|t| (t.min(MAX_FREQ_RATIO) * PI).tan()));

    &TABLE
}

#[cfg(test)]
mod tests;

mod response;
mod stage;

pub use response::SvfResponse;

use stage::{OnePoleStage, SvfStage};

#[inline(always)]
fn q_from_resonance(resonance: Sample) -> Sample {
    ZERO_RESONANCE_Q + (MAX_RESONANCE_Q - ZERO_RESONANCE_Q) * resonance * resonance * resonance
}

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
}

#[derive(Clone, Copy, Default)]
struct LowPass;

#[derive(Clone, Copy, Default)]
struct HighPass;

#[derive(Clone, Copy, Default)]
struct BandPass;

#[enum_dispatch]
pub(crate) trait SvfFilter {
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample;
    fn reset(&mut self);

    fn process(
        &mut self,
        sample_rate: Sample,
        input: &[Sample],
        cutoff: &[Sample],
        resonance: &[Sample],
        gain: &[Sample],
        output: &mut [Sample],
    ) {
        let g_table = g_table();
        let inv_sample_rate = sample_rate.recip();

        for (out, &sample, &cutoff, &resonance, &gain) in
            izip!(output, input, cutoff, resonance, gain)
        {
            let freq = pitch_to_freq(C4_PITCH + cutoff);
            let g = g_table.at((freq * inv_sample_rate).min(MAX_FREQ_RATIO));
            let k = q_from_resonance(resonance).recip();

            *out = self.tick(g, k, sample * gain);
        }
    }
}

#[derive(Default, Clone, Copy)]
pub(crate) struct LowPass12 {
    resonant: SvfStage<LowPass>,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct LowPass18 {
    resonant: SvfStage<LowPass>,
    one_pole: OnePoleStage<LowPass>,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct LowPass24 {
    fixed: SvfStage<LowPass>,
    resonant: SvfStage<LowPass>,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct HighPass12 {
    resonant: SvfStage<HighPass>,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct HighPass18 {
    resonant: SvfStage<HighPass>,
    one_pole: OnePoleStage<HighPass>,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct HighPass24 {
    fixed: SvfStage<HighPass>,
    resonant: SvfStage<HighPass>,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct BandPass6 {
    resonant: SvfStage<BandPass>,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct BandPass12 {
    fixed: SvfStage<BandPass>,
    resonant: SvfStage<BandPass>,
}

impl SvfFilter for LowPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for LowPass18 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.resonant.tick(g, k, input);

        self.one_pole.tick(g, x)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for LowPass24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input);

        self.resonant.tick(g, k, x)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for HighPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for HighPass18 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.resonant.tick(g, k, input);

        self.one_pole.tick(g, x)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for HighPass24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input);

        self.resonant.tick(g, k, x)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for BandPass6 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl SvfFilter for BandPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input);

        self.resonant.tick(g, k, x)
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
        }
    }

    /// Replaces the variant when `filter_type` changes, which clears integrator state.
    pub(crate) fn set_type(&mut self, filter_type: SvfType) {
        if self.current_type() != filter_type {
            *self = Self::new(filter_type);
        }
    }
}
