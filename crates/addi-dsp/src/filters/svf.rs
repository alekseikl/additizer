//! Time-domain state-variable filter in the Cytomic / Andrew Simper form

use enum_dispatch::enum_dispatch;
use itertools::izip;
use serde::{Deserialize, Serialize};
use wide::f32x4;

use crate::{
    Sample,
    units::{C4_PITCH, fast_pitch_to_freq_x4, fast_tan_pi_x4},
};

/// Largest `f / sample_rate` fed to `tan`, keeping `g` finite just below Nyquist.
const MAX_FREQ_RATIO: Sample = 0.499;

#[cfg(test)]
mod tests;

mod response;
mod sections;

pub use response::SvfResponse;

use sections::{
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

/// Coefficients and input for one sample.
pub struct TickParams {
    g: Sample,
    /// `1/Q` of the resonant stage.
    k: Sample,
    /// `1/Q` of the pre stage.
    pre_k: Sample,
    gain: Sample,
    input: Sample,
}

#[enum_dispatch]
pub trait SvfFilter {
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
        let (output, output_tail) = output.as_chunks_mut::<LANES>();
        let done = output.len() * LANES;

        for (output, cutoff, k, pre_k, gain, input) in izip!(
            output,
            cutoff.as_chunks::<LANES>().0,
            k.as_chunks::<LANES>().0,
            pre_k.as_chunks::<LANES>().0,
            gain.as_chunks::<LANES>().0,
            input.as_chunks::<LANES>().0,
        ) {
            let g = cutoff_to_g_x4(f32x4::new(*cutoff), freq_mult).to_array();

            for (sample, g, &k, &pre_k, &gain, &input) in izip!(output, g, k, pre_k, gain, input) {
                *sample = self.tick(&TickParams {
                    g,
                    k,
                    pre_k,
                    gain,
                    input,
                });
            }
        }

        let tail = output_tail.len();

        if tail > 0 {
            let mut cutoff_lanes = [0.0; LANES];
            cutoff_lanes[..tail].copy_from_slice(&cutoff[done..done + tail]);
            let g = cutoff_to_g_x4(f32x4::new(cutoff_lanes), freq_mult).to_array();

            for (sample, g, &k, &pre_k, &gain, &input) in izip!(
                output_tail,
                g,
                &k[done..],
                &pre_k[done..],
                &gain[done..],
                &input[done..],
            ) {
                *sample = self.tick(&TickParams {
                    g,
                    k,
                    pre_k,
                    gain,
                    input,
                });
            }
        }
    }
}

const LANES: usize = 4;

/// `g = tan(π f / sample_rate)` for four cutoffs in octaves relative to C4.
#[inline(always)]
fn cutoff_to_g_x4(cutoff: f32x4, freq_mult: Sample) -> f32x4 {
    const MAX_RATIO: f32x4 = f32x4::splat(MAX_FREQ_RATIO);
    const C4: f32x4 = f32x4::splat(C4_PITCH);

    let freq = fast_pitch_to_freq_x4(cutoff + C4);
    let ratio = (freq * f32x4::splat(freq_mult)).fast_min(MAX_RATIO);

    fast_tan_pi_x4(ratio)
}

#[derive(Default, Clone, Copy)]
pub struct LowPass12 {
    resonant: LowPass,
}

#[derive(Default, Clone, Copy)]
pub struct LowPass18 {
    resonant: LowPass,
    one_pole: OnePoleLowPass,
}

#[derive(Default, Clone, Copy)]
pub struct LowPass24 {
    pre_stage: LowPass,
    resonant: LowPass,
}

#[derive(Default, Clone, Copy)]
pub struct HighPass12 {
    resonant: HighPass,
}

#[derive(Default, Clone, Copy)]
pub struct HighPass18 {
    resonant: HighPass,
    one_pole: OnePoleHighPass,
}

#[derive(Default, Clone, Copy)]
pub struct HighPass24 {
    pre_stage: HighPass,
    resonant: HighPass,
}

#[derive(Default, Clone, Copy)]
pub struct BandPass6 {
    resonant: BandPass,
}

#[derive(Default, Clone, Copy)]
pub struct BandPass12 {
    pre_stage: BandPass,
    resonant: BandPass,
}

/// 18 dB/oct. Two pre stages at `pre_k`, then the resonant stage.
#[derive(Default, Clone, Copy)]
pub struct BandPass18 {
    pre_stages: [BandPass; 2],
    resonant: BandPass,
}

/// 24 dB/oct. Three pre stages at `pre_k`, then the resonant stage.
#[derive(Default, Clone, Copy)]
pub struct BandPass24 {
    pre_stages: [BandPass; 3],
    resonant: BandPass,
}

/// Bell. Linear `gain` is the level at the cutoff; DC and high frequencies stay at unity.
#[derive(Default, Clone, Copy)]
pub struct Peaking {
    section: sections::Peaking,
}

#[derive(Default, Clone, Copy)]
pub struct Notch {
    resonant: sections::Notch,
}

/// Low shelf, 12 dB/oct. Linear `gain` is the DC level; high frequencies stay at unity.
#[derive(Default, Clone, Copy)]
pub struct LowShelf12 {
    section: LowShelf,
}

/// High shelf, 12 dB/oct. Linear `gain` is the high-frequency level; DC stays at unity.
#[derive(Default, Clone, Copy)]
pub struct HighShelf12 {
    section: HighShelf,
}

/// Low shelf, 24 dB/oct. Two 12 dB sections, each at `sqrt(gain)`. One stage uses `pre_k`.
#[derive(Default, Clone, Copy)]
pub struct LowShelf24 {
    pre_stage: LowShelf,
    resonant: LowShelf,
}

/// High shelf, 24 dB/oct. Two 12 dB sections, each at `sqrt(gain)`. One stage uses `pre_k`.
#[derive(Default, Clone, Copy)]
pub struct HighShelf24 {
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

impl SvfFilter for BandPass18 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        let x = self.pre_stages[0].tick(p.g, p.pre_k, p.input * p.gain);
        let x = self.pre_stages[1].tick(p.g, p.pre_k, x);

        self.resonant.tick(p.g, p.k, x)
    }
}

impl SvfFilter for BandPass24 {
    #[inline(always)]
    fn tick(&mut self, p: &TickParams) -> Sample {
        let x = self.pre_stages[0].tick(p.g, p.pre_k, p.input * p.gain);
        let x = self.pre_stages[1].tick(p.g, p.pre_k, x);
        let x = self.pre_stages[2].tick(p.g, p.pre_k, x);

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
pub enum SvfState {
    LowPass12(LowPass12),
    LowPass18(LowPass18),
    LowPass24(LowPass24),
    HighPass12(HighPass12),
    HighPass18(HighPass18),
    HighPass24(HighPass24),
    BandPass6(BandPass6),
    BandPass12(BandPass12),
    BandPass18(BandPass18),
    BandPass24(BandPass24),
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
            SvfType::BandPass18 => Self::BandPass18(BandPass18::default()),
            SvfType::BandPass24 => Self::BandPass24(BandPass24::default()),
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
            Self::BandPass18(_) => SvfType::BandPass18,
            Self::BandPass24(_) => SvfType::BandPass24,
            Self::Peaking(_) => SvfType::Peaking,
            Self::Notch(_) => SvfType::Notch,
            Self::LowShelf12(_) => SvfType::LowShelf12,
            Self::LowShelf24(_) => SvfType::LowShelf24,
            Self::HighShelf12(_) => SvfType::HighShelf12,
            Self::HighShelf24(_) => SvfType::HighShelf24,
        }
    }

    /// Replaces the variant when `filter_type` changes, which clears integrator state.
    pub fn set_type(&mut self, filter_type: SvfType) {
        if self.current_type() != filter_type {
            *self = Self::new(filter_type);
        }
    }
}
