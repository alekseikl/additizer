//! Time-domain state-variable filter in the Cytomic / Andrew Simper form

use std::{f32::consts::PI, marker::PhantomData};

use enum_dispatch::enum_dispatch;
use itertools::izip;
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ComplexSample, Sample},
    utils::{C4_PITCH, pitch_to_freq},
};

pub const MIN_RESONANCE: Sample = 0.0;
pub const MAX_RESONANCE: Sample = 1.0;

const ZERO_RESONANCE_Q: Sample = 0.5;
const MAX_RESONANCE_Q: Sample = 16.0;

/// Resonance in [`MIN_RESONANCE`, `MAX_RESONANCE`] mapped to Q with a cubic curve.
#[inline(always)]
fn q_from_resonance(resonance: Sample) -> Sample {
    ZERO_RESONANCE_Q + (MAX_RESONANCE_Q - ZERO_RESONANCE_Q) * resonance * resonance * resonance
}

#[cfg(test)]
mod tests;

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

impl SvfType {
    pub const ALL: [Self; 8] = [
        Self::LowPass12,
        Self::LowPass18,
        Self::LowPass24,
        Self::HighPass12,
        Self::HighPass18,
        Self::HighPass24,
        Self::BandPass6,
        Self::BandPass12,
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
        }
    }
}

/// Output tap of [`SvfStage`]. Monomorphizing on this (`SvfStage<LowPass>`)
/// selects the tap at compile time.
trait Shape: Copy {
    fn output(v0: Sample, v1: Sample, v2: Sample, k: Sample) -> Sample;
    fn one_pole(x: Sample, lp: Sample) -> Sample;
    fn response(k: Sample, s: ComplexSample) -> ComplexSample;
    fn one_pole_response(s: ComplexSample) -> ComplexSample;
}

#[derive(Clone, Copy, Default)]
struct LowPass;

#[derive(Clone, Copy, Default)]
struct HighPass;

#[derive(Clone, Copy, Default)]
struct BandPass;

impl Shape for LowPass {
    #[inline(always)]
    fn output(_v0: Sample, _v1: Sample, v2: Sample, _k: Sample) -> Sample {
        v2
    }

    #[inline(always)]
    fn one_pole(_x: Sample, lp: Sample) -> Sample {
        lp
    }

    #[inline(always)]
    fn response(k: Sample, s: ComplexSample) -> ComplexSample {
        (s * s + s * k + 1.0).inv()
    }

    #[inline(always)]
    fn one_pole_response(s: ComplexSample) -> ComplexSample {
        (s + 1.0).inv()
    }
}

impl Shape for HighPass {
    #[inline(always)]
    fn output(v0: Sample, v1: Sample, v2: Sample, k: Sample) -> Sample {
        v0 - k * v1 - v2
    }

    #[inline(always)]
    fn one_pole(x: Sample, lp: Sample) -> Sample {
        x - lp
    }

    #[inline(always)]
    fn response(k: Sample, s: ComplexSample) -> ComplexSample {
        let d = s * s + s * k + 1.0;

        s * s / d
    }

    #[inline(always)]
    fn one_pole_response(s: ComplexSample) -> ComplexSample {
        s / (s + 1.0)
    }
}

impl Shape for BandPass {
    #[inline(always)]
    fn output(_v0: Sample, v1: Sample, _v2: Sample, k: Sample) -> Sample {
        k * v1
    }

    #[inline(always)]
    fn one_pole(_x: Sample, lp: Sample) -> Sample {
        lp
    }

    #[inline(always)]
    fn response(k: Sample, s: ComplexSample) -> ComplexSample {
        let d = s * s + s * k + 1.0;

        s * k / d
    }

    #[inline(always)]
    fn one_pole_response(s: ComplexSample) -> ComplexSample {
        (s + 1.0).inv()
    }
}

/// One SVF section. `S` is the output tap, as in `SvfStage<LowPass>`.
#[derive(Default, Clone, Copy)]
struct SvfStage<S> {
    ic1eq: Sample,
    ic2eq: Sample,
    _shape: PhantomData<S>,
}

impl<S> SvfStage<S> {
    fn is_finite(&self) -> bool {
        self.ic1eq.is_finite() && self.ic2eq.is_finite()
    }
}

impl<S: Shape> SvfStage<S> {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, v0: Sample) -> Sample {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * (self.ic1eq + g * v3);
        let v2 = self.ic2eq + g * v1;

        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        S::output(v0, v1, v2, k)
    }
}

#[derive(Default, Clone, Copy)]
struct OnePoleStage<S> {
    s: Sample,
    _shape: PhantomData<S>,
}

impl<S> OnePoleStage<S> {
    fn is_finite(&self) -> bool {
        self.s.is_finite()
    }
}

impl<S: Shape> OnePoleStage<S> {
    #[inline(always)]
    fn tick(&mut self, g: Sample, x: Sample) -> Sample {
        let g = g / (1.0 + g);
        let v = (x - self.s) * g;
        let lp = v + self.s;

        self.s = lp + v;

        S::one_pole(x, lp)
    }
}

fn response_2<S: Shape>(q: Sample, s: ComplexSample) -> ComplexSample {
    S::response(q.recip(), s)
}

fn response_3<S: Shape>(q: Sample, s: ComplexSample) -> ComplexSample {
    S::response(q.recip(), s) * S::one_pole_response(s)
}

fn response_4<S: Shape>(q: Sample, s: ComplexSample) -> ComplexSample {
    S::response(q.recip(), s) * S::response(1.0, s)
}

#[enum_dispatch]
pub(crate) trait SvfFilter {
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample;
    fn is_finite(&self) -> bool;
    fn reset(&mut self);
    fn response(&self, q: Sample, s: ComplexSample) -> ComplexSample;

    fn process(
        &mut self,
        sample_rate: Sample,
        input: &[Sample],
        cutoff: &[Sample],
        resonance: &[Sample],
        gain: &[Sample],
        output: &mut [Sample],
    ) {
        const MAX_OMEGA: Sample = 0.499 * PI;
        let pi_over_rate = PI / sample_rate;

        for (out, &sample, &cutoff, &resonance, &gain) in
            izip!(output, input, cutoff, resonance, gain)
        {
            let freq = pitch_to_freq(C4_PITCH + cutoff);
            let g = (freq * pi_over_rate).min(MAX_OMEGA).tan();
            let k = q_from_resonance(resonance).recip();

            *out = self.tick(g, k, sample * gain);
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct LowPass12 {
    resonant: SvfStage<LowPass>,
}

#[derive(Default, Clone, Copy)]
pub struct LowPass18 {
    resonant: SvfStage<LowPass>,
    one_pole: OnePoleStage<LowPass>,
}

#[derive(Default, Clone, Copy)]
pub struct LowPass24 {
    fixed: SvfStage<LowPass>,
    resonant: SvfStage<LowPass>,
}

#[derive(Default, Clone, Copy)]
pub struct HighPass12 {
    resonant: SvfStage<HighPass>,
}

#[derive(Default, Clone, Copy)]
pub struct HighPass18 {
    resonant: SvfStage<HighPass>,
    one_pole: OnePoleStage<HighPass>,
}

#[derive(Default, Clone, Copy)]
pub struct HighPass24 {
    fixed: SvfStage<HighPass>,
    resonant: SvfStage<HighPass>,
}

#[derive(Default, Clone, Copy)]
pub struct BandPass6 {
    resonant: SvfStage<BandPass>,
}

#[derive(Default, Clone, Copy)]
pub struct BandPass12 {
    fixed: SvfStage<BandPass>,
    resonant: SvfStage<BandPass>,
}

impl SvfFilter for LowPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input)
    }

    fn is_finite(&self) -> bool {
        self.resonant.is_finite()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn response(&self, q: Sample, s: ComplexSample) -> ComplexSample {
        response_2::<LowPass>(q, s)
    }
}

impl SvfFilter for LowPass18 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.resonant.tick(g, k, input);

        self.one_pole.tick(g, x)
    }

    fn is_finite(&self) -> bool {
        self.resonant.is_finite() && self.one_pole.is_finite()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn response(&self, q: Sample, s: ComplexSample) -> ComplexSample {
        response_3::<LowPass>(q, s)
    }
}

impl SvfFilter for LowPass24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input);

        self.resonant.tick(g, k, x)
    }

    fn is_finite(&self) -> bool {
        self.fixed.is_finite() && self.resonant.is_finite()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn response(&self, q: Sample, s: ComplexSample) -> ComplexSample {
        response_4::<LowPass>(q, s)
    }
}

impl SvfFilter for HighPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input)
    }

    fn is_finite(&self) -> bool {
        self.resonant.is_finite()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn response(&self, q: Sample, s: ComplexSample) -> ComplexSample {
        response_2::<HighPass>(q, s)
    }
}

impl SvfFilter for HighPass18 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.resonant.tick(g, k, input);

        self.one_pole.tick(g, x)
    }

    fn is_finite(&self) -> bool {
        self.resonant.is_finite() && self.one_pole.is_finite()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn response(&self, q: Sample, s: ComplexSample) -> ComplexSample {
        response_3::<HighPass>(q, s)
    }
}

impl SvfFilter for HighPass24 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input);

        self.resonant.tick(g, k, x)
    }

    fn is_finite(&self) -> bool {
        self.fixed.is_finite() && self.resonant.is_finite()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn response(&self, q: Sample, s: ComplexSample) -> ComplexSample {
        response_4::<HighPass>(q, s)
    }
}

impl SvfFilter for BandPass6 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        self.resonant.tick(g, k, input)
    }

    fn is_finite(&self) -> bool {
        self.resonant.is_finite()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn response(&self, q: Sample, s: ComplexSample) -> ComplexSample {
        response_2::<BandPass>(q, s)
    }
}

impl SvfFilter for BandPass12 {
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let x = self.fixed.tick(g, 1.0, input);

        self.resonant.tick(g, k, x)
    }

    fn is_finite(&self) -> bool {
        self.fixed.is_finite() && self.resonant.is_finite()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn response(&self, q: Sample, s: ComplexSample) -> ComplexSample {
        response_4::<BandPass>(q, s)
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

/// Frequency response of the filter as a function of the normalized complex
/// frequency `s` (`s = jω / ω_cutoff` for the analog prototype).
#[derive(Clone, Copy)]
pub struct SvfResponse {
    pub filter_type: SvfType,
    pub resonance: Sample,
}

impl SvfResponse {
    /// Normalized `s` of the analog prototype (no frequency warping).
    pub fn analog_s(freq: Sample, cutoff: Sample) -> ComplexSample {
        ComplexSample::new(0.0, freq / cutoff)
    }

    /// Normalized `s` of the trapezoidal (bilinear) discretization at `sample_rate`.
    /// This is exactly what [`SvfFilter::tick`] implements.
    pub fn digital_s(freq: Sample, cutoff: Sample, sample_rate: Sample) -> ComplexSample {
        let w = (PI * freq / sample_rate).tan();
        let g = (PI * cutoff / sample_rate).tan();

        ComplexSample::new(0.0, w / g)
    }

    pub fn at(&self, s: ComplexSample) -> ComplexSample {
        let q = q_from_resonance(self.resonance);

        SvfState::new(self.filter_type).response(q, s)
    }
}
