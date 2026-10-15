//! Time-domain state-variable filter in the Cytomic / Andrew Simper form.

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

use crate::{
    SIMD_LANES, Smp, WideSmp,
    units::{C4_PITCH, fast_pitch_to_freq_x4, fast_tan_pi_x4},
};

/// Largest `f / sample_rate` fed to `tan`, keeping `g` finite just below Nyquist.
const MAX_FREQ_RATIO: Smp = 0.499;

#[cfg(test)]
mod tests;

mod response;
mod sections;

pub use response::SvfResponse;

use sections::{
    BandPass, HighPass, HighShelf, LowPass, LowShelf, Notch, OnePoleHighPass, OnePoleLowPass,
    Peaking, ShelfCoeffs,
};

/// Four lanes: even-left, even-right, odd-left, odd-right.
const LANES: usize = SIMD_LANES;

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
    BandPass18,
    BandPass24,
    Peaking,
    Notch,
    LowShelf12,
    LowShelf24,
    HighShelf12,
    HighShelf24,
}

impl SvfType {
    pub const ALL: [Self; 16] = [
        Self::LowPass12,
        Self::LowPass18,
        Self::LowPass24,
        Self::HighPass12,
        Self::HighPass18,
        Self::HighPass24,
        Self::BandPass6,
        Self::BandPass12,
        Self::BandPass18,
        Self::BandPass24,
        Self::Peaking,
        Self::Notch,
        Self::LowShelf12,
        Self::LowShelf24,
        Self::HighShelf12,
        Self::HighShelf24,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::LowPass12 => "Lowpass 12",
            Self::LowPass18 => "Lowpass 18",
            Self::LowPass24 => "Lowpass 24",
            Self::HighPass12 => "Highpass 12",
            Self::HighPass18 => "Highpass 18",
            Self::HighPass24 => "Highpass 24",
            Self::BandPass6 => "Bandpass 6",
            Self::BandPass12 => "Bandpass 12",
            Self::BandPass18 => "Bandpass 18",
            Self::BandPass24 => "Bandpass 24",
            Self::Peaking => "Peaking",
            Self::Notch => "Notch",
            Self::LowShelf12 => "Lowshelf 12",
            Self::LowShelf24 => "Lowshelf 24",
            Self::HighShelf12 => "Highshelf 12",
            Self::HighShelf24 => "Highshelf 24",
        }
    }
}

struct LaneParams {
    g: WideSmp,
    k: WideSmp,
    pre_k: WideSmp,
    gain: WideSmp,
    input: WideSmp,
}

#[enum_dispatch]
trait SvfFilter {
    fn tick(&mut self, p: &LaneParams) -> WideSmp;

    fn reset(&mut self, mask: WideSmp);

    fn process(
        &mut self,
        sample_rate: Smp,
        samples: usize,
        inputs: LaneInputs<'_>,
        output: [&mut [Smp]; LANES],
    ) {
        let freq_mult = WideSmp::splat(sample_rate.recip());
        let inputs = inputs.take(samples);
        let [o0, o1, o2, o3] = take_lanes_mut(output, samples);
        let samples = o0.len();

        for index in 0..samples {
            let y = self.tick(&inputs.params(freq_mult, index)).to_array();

            o0[index] = y[0];
            o1[index] = y[1];
            o2[index] = y[2];
            o3[index] = y[3];
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct LowPass12 {
    resonant: LowPass,
}

impl SvfFilter for LowPass12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct LowPass18 {
    resonant: LowPass,
    one_pole: OnePoleLowPass,
}

impl SvfFilter for LowPass18 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        let x = self.resonant.tick(p.g, p.k, p.input * p.gain);
        self.one_pole.tick(p.g, x)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.resonant.clear(mask);
        self.one_pole.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct LowPass24 {
    pre_stage: LowPass,
    resonant: LowPass,
}

impl SvfFilter for LowPass24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        let x = self.pre_stage.tick(p.g, p.pre_k, p.input * p.gain);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct HighPass12 {
    resonant: HighPass,
}

impl SvfFilter for HighPass12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct HighPass18 {
    resonant: HighPass,
    one_pole: OnePoleHighPass,
}

impl SvfFilter for HighPass18 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        let x = self.resonant.tick(p.g, p.k, p.input * p.gain);
        self.one_pole.tick(p.g, x)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.resonant.clear(mask);
        self.one_pole.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct HighPass24 {
    pre_stage: HighPass,
    resonant: HighPass,
}

impl SvfFilter for HighPass24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        let x = self.pre_stage.tick(p.g, p.pre_k, p.input * p.gain);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct BandPass6 {
    resonant: BandPass,
}

impl SvfFilter for BandPass6 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct BandPass12 {
    pre_stage: BandPass,
    resonant: BandPass,
}

impl SvfFilter for BandPass12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        let x = self.pre_stage.tick(p.g, p.pre_k, p.input * p.gain);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct BandPass18 {
    pre_stages: [BandPass; 2],
    resonant: BandPass,
}

impl SvfFilter for BandPass18 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        let x = self.pre_stages[0].tick(p.g, p.pre_k, p.input * p.gain);
        let x = self.pre_stages[1].tick(p.g, p.pre_k, x);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.pre_stages[0].clear(mask);
        self.pre_stages[1].clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct BandPass24 {
    pre_stages: [BandPass; 3],
    resonant: BandPass,
}

impl SvfFilter for BandPass24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        let x = self.pre_stages[0].tick(p.g, p.pre_k, p.input * p.gain);
        let x = self.pre_stages[1].tick(p.g, p.pre_k, x);
        let x = self.pre_stages[2].tick(p.g, p.pre_k, x);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.pre_stages[0].clear(mask);
        self.pre_stages[1].clear(mask);
        self.pre_stages[2].clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct PeakingFilter {
    section: Peaking,
}

impl SvfFilter for PeakingFilter {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        self.section.tick(p.g, p.k, p.gain, p.input)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.section.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct NotchFilter {
    resonant: Notch,
}

impl SvfFilter for NotchFilter {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct LowShelf12 {
    section: LowShelf,
}

impl SvfFilter for LowShelf12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        self.section
            .tick(p.g, p.k, ShelfCoeffs::new(p.gain), p.input)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.section.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct HighShelf12 {
    section: HighShelf,
}

impl SvfFilter for HighShelf12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        self.section
            .tick(p.g, p.k, ShelfCoeffs::new(p.gain), p.input)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.section.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct LowShelf24 {
    pre_stage: LowShelf,
    resonant: LowShelf,
}

impl SvfFilter for LowShelf24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        let coeffs = ShelfCoeffs::cascaded(p.gain);
        let x = self.pre_stage.tick(p.g, p.pre_k, coeffs, p.input);
        self.resonant.tick(p.g, p.k, coeffs, x)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub struct HighShelf24 {
    pre_stage: HighShelf,
    resonant: HighShelf,
}

impl SvfFilter for HighShelf24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> WideSmp {
        let coeffs = ShelfCoeffs::cascaded(p.gain);
        let x = self.pre_stage.tick(p.g, p.pre_k, coeffs, p.input);
        self.resonant.tick(p.g, p.k, coeffs, x)
    }

    fn reset(&mut self, mask: WideSmp) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

#[enum_dispatch(SvfFilter)]
#[derive(Clone, Copy)]
pub enum SvfState {
    LowPass12,
    LowPass18,
    LowPass24,
    HighPass12,
    HighPass18,
    HighPass24,
    BandPass6,
    BandPass12,
    BandPass18,
    BandPass24,
    PeakingFilter,
    NotchFilter,
    LowShelf12,
    LowShelf24,
    HighShelf12,
    HighShelf24,
}

impl Default for SvfState {
    fn default() -> Self {
        Self::new(SvfType::LowPass12)
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
            SvfType::BandPass18 => Self::BandPass18(BandPass18::default()),
            SvfType::BandPass24 => Self::BandPass24(BandPass24::default()),
            SvfType::Peaking => Self::PeakingFilter(PeakingFilter::default()),
            SvfType::Notch => Self::NotchFilter(NotchFilter::default()),
            SvfType::LowShelf12 => Self::LowShelf12(LowShelf12::default()),
            SvfType::LowShelf24 => Self::LowShelf24(LowShelf24::default()),
            SvfType::HighShelf12 => Self::HighShelf12(HighShelf12::default()),
            SvfType::HighShelf24 => Self::HighShelf24(HighShelf24::default()),
        }
    }

    fn filter_type(self) -> SvfType {
        match self {
            Self::LowPass12(_) => SvfType::LowPass12,
            Self::LowPass18(_) => SvfType::LowPass18,
            Self::LowPass24(_) => SvfType::LowPass24,
            Self::HighPass12(_) => SvfType::HighPass12,
            Self::HighPass18(_) => SvfType::HighPass18,
            Self::HighPass24(_) => SvfType::HighPass24,
            Self::BandPass6(_) => SvfType::BandPass6,
            Self::BandPass12(_) => SvfType::BandPass12,
            Self::BandPass18(_) => SvfType::BandPass18,
            Self::BandPass24(_) => SvfType::BandPass24,
            Self::PeakingFilter(_) => SvfType::Peaking,
            Self::NotchFilter(_) => SvfType::Notch,
            Self::LowShelf12(_) => SvfType::LowShelf12,
            Self::LowShelf24(_) => SvfType::LowShelf24,
            Self::HighShelf12(_) => SvfType::HighShelf12,
            Self::HighShelf24(_) => SvfType::HighShelf24,
        }
    }

    /// Replaces the variant when `filter_type` changes, which clears every lane.
    pub fn set_type(&mut self, filter_type: SvfType) {
        if self.filter_type() != filter_type {
            *self = Self::new(filter_type);
        }
    }

    /// Zeros one voice's lanes. `slot` is 0 for lanes 0–1 and 1 for lanes 2–3.
    pub fn reset(&mut self, slot: usize) {
        let on = f32::from_bits(u32::MAX);
        let mask = match slot {
            0 => WideSmp::new([on, on, 0.0, 0.0]),
            1 => WideSmp::new([0.0, 0.0, on, on]),
            _ => panic!("voice slot in a pair is 0 or 1"),
        };

        SvfFilter::reset(self, mask);
    }

    pub fn process(
        &mut self,
        sample_rate: Smp,
        samples: usize,
        inputs: LaneInputs<'_>,
        output: [&mut [Smp]; LANES],
    ) {
        SvfFilter::process(self, sample_rate, samples, inputs, output);
    }
}

#[inline(always)]
fn take_lanes_mut(lanes: [&mut [Smp]; LANES], samples: usize) -> [&mut [Smp]; LANES] {
    lanes.map(|lane| &mut lane[..samples])
}

#[derive(Clone, Copy)]
pub struct LaneInputs<'a> {
    pub audio: [&'a [Smp]; LANES],
    pub cutoff: [&'a [Smp]; LANES],
    pub k: [&'a [Smp]; LANES],
    pub pre_k: [&'a [Smp]; LANES],
    pub gain: [&'a [Smp]; LANES],
}

impl LaneInputs<'_> {
    #[inline(always)]
    fn take(&self, samples: usize) -> Self {
        Self {
            audio: self.audio.map(|lane| &lane[..samples]),
            cutoff: self.cutoff.map(|lane| &lane[..samples]),
            k: self.k.map(|lane| &lane[..samples]),
            pre_k: self.pre_k.map(|lane| &lane[..samples]),
            gain: self.gain.map(|lane| &lane[..samples]),
        }
    }

    /// `g = tan(π f / sample_rate)` for four cutoffs in octaves relative to C4.
    #[inline(always)]
    fn cutoff_to_g_x4(cutoff: WideSmp, freq_mult: WideSmp) -> WideSmp {
        const MAX_RATIO: WideSmp = WideSmp::splat(MAX_FREQ_RATIO);
        const C4: WideSmp = WideSmp::splat(C4_PITCH);

        let freq = fast_pitch_to_freq_x4(cutoff + C4);
        let ratio = (freq * freq_mult).fast_min(MAX_RATIO);

        fast_tan_pi_x4(ratio)
    }

    #[inline(always)]
    fn load(lanes: [&[Smp]; LANES], index: usize) -> WideSmp {
        WideSmp::new([
            lanes[0][index],
            lanes[1][index],
            lanes[2][index],
            lanes[3][index],
        ])
    }

    #[inline(always)]
    fn params(&self, freq_mult: WideSmp, index: usize) -> LaneParams {
        LaneParams {
            g: Self::cutoff_to_g_x4(Self::load(self.cutoff, index), freq_mult),
            k: Self::load(self.k, index),
            pre_k: Self::load(self.pre_k, index),
            gain: Self::load(self.gain, index),
            input: Self::load(self.audio, index),
        }
    }
}
