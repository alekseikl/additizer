//! Time-domain state-variable filter in the Cytomic / Andrew Simper form
//! ("Solving the continuous SVF equations using trapezoidal integration and
//! equivalent currents"), plus a trapezoidal (TPT) one-pole section that is
//! cascaded after the SVF for the 18 dB variants.
//!
//! Pole layout:
//!
//! | Slope | Stages                                             |
//! | ----- | -------------------------------------------------- |
//! | 12 dB | resonant SVF                                       |
//! | 18 dB | resonant SVF → one-pole (LP, or HP for high-pass)  |
//! | 24 dB | Butterworth SVF → resonant SVF                     |
//!
//! Only one stage carries the user resonance, so the peak gain does not grow
//! with the pole count (same approach as the spectral filter's 24 dB modes).

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

use crate::synth_engine::{ComplexSample, Sample, filters::spectral_filter::BUTTERWORTH_Q};

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

/// Which SVF output a stage produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Low,
    High,
    Band,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Poles {
    Two,
    Three,
    Four,
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

    fn shape(self) -> Shape {
        match self {
            Self::LowPass12 | Self::LowPass18 | Self::LowPass24 => Shape::Low,
            Self::HighPass12 | Self::HighPass18 | Self::HighPass24 => Shape::High,
            Self::BandPass6 | Self::BandPass12 => Shape::Band,
        }
    }

    fn poles(self) -> Poles {
        match self {
            Self::LowPass12 | Self::HighPass12 | Self::BandPass6 => Poles::Two,
            Self::LowPass18 | Self::HighPass18 => Poles::Three,
            Self::LowPass24 | Self::HighPass24 | Self::BandPass12 => Poles::Four,
        }
    }
}

#[derive(Clone, Copy)]
struct StageCoeffs {
    k: Sample,
    a1: Sample,
    a2: Sample,
    a3: Sample,
}

impl StageCoeffs {
    #[inline]
    fn new(g: Sample, k: Sample) -> Self {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        Self { k, a1, a2, a3 }
    }
}

/// Per-sample coefficients shared by all stages of one filter instance.
#[derive(Clone, Copy)]
pub struct SvfCoeffs {
    resonant: StageCoeffs,
    butterworth: StageCoeffs,
    /// TPT one-pole gain `G = g / (1 + g)`.
    one_pole_g: Sample,
}

impl SvfCoeffs {
    /// `cutoff` in Hz, must be below `sample_rate / 2`.
    pub fn new(cutoff: Sample, q: Sample, sample_rate: Sample) -> Self {
        let g = (PI * cutoff / sample_rate).tan();

        Self {
            resonant: StageCoeffs::new(g, q.recip()),
            butterworth: StageCoeffs::new(g, BUTTERWORTH_Q.recip()),
            one_pole_g: g / (1.0 + g),
        }
    }
}

#[derive(Default, Clone, Copy)]
struct SvfStage {
    ic1eq: Sample,
    ic2eq: Sample,
}

impl SvfStage {
    /// Advances the integrators and returns the selected output.
    #[inline(always)]
    fn tick(&mut self, c: &StageCoeffs, shape: Shape, v0: Sample) -> Sample {
        let v3 = v0 - self.ic2eq;
        let v1 = c.a1 * self.ic1eq + c.a2 * v3;
        let v2 = self.ic2eq + c.a2 * self.ic1eq + c.a3 * v3;

        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        match shape {
            Shape::Low => v2,
            // Normalized band-pass: unity gain at the cutoff for any resonance.
            Shape::Band => c.k * v1,
            Shape::High => v0 - c.k * v1 - v2,
        }
    }

    fn is_finite(&self) -> bool {
        self.ic1eq.is_finite() && self.ic2eq.is_finite()
    }
}

#[derive(Default, Clone, Copy)]
struct OnePoleStage {
    s: Sample,
}

impl OnePoleStage {
    #[inline(always)]
    fn tick(&mut self, g: Sample, shape: Shape, x: Sample) -> Sample {
        let v = (x - self.s) * g;
        let lp = v + self.s;

        self.s = lp + v;

        match shape {
            Shape::High => x - lp,
            Shape::Low | Shape::Band => lp,
        }
    }
}

/// Per-voice, per-channel filter memory.
#[derive(Default, Clone, Copy)]
pub struct SvfState {
    resonant: SvfStage,
    butterworth: SvfStage,
    one_pole: OnePoleStage,
}

impl SvfState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_finite(&self) -> bool {
        self.resonant.is_finite() && self.butterworth.is_finite() && self.one_pole.s.is_finite()
    }

    #[inline]
    pub fn tick(&mut self, filter_type: SvfType, c: &SvfCoeffs, input: Sample) -> Sample {
        let shape = filter_type.shape();
        let poles = filter_type.poles();
        let x = match poles {
            Poles::Four => self.butterworth.tick(&c.butterworth, shape, input),
            Poles::Two | Poles::Three => input,
        };
        let x = self.resonant.tick(&c.resonant, shape, x);

        match poles {
            Poles::Three => self.one_pole.tick(c.one_pole_g, shape, x),
            Poles::Two | Poles::Four => x,
        }
    }
}

/// Frequency response of the filter as a function of the normalized complex
/// frequency `s` (`s = jω / ω_cutoff` for the analog prototype).
#[derive(Clone, Copy)]
pub struct SvfResponse {
    pub filter_type: SvfType,
    pub q: Sample,
}

impl SvfResponse {
    /// Normalized `s` of the analog prototype (no frequency warping).
    pub fn analog_s(freq: Sample, cutoff: Sample) -> ComplexSample {
        ComplexSample::new(0.0, freq / cutoff)
    }

    /// Normalized `s` of the trapezoidal (bilinear) discretization at `sample_rate`.
    /// This is exactly what [`SvfState::tick`] implements.
    pub fn digital_s(freq: Sample, cutoff: Sample, sample_rate: Sample) -> ComplexSample {
        let w = (PI * freq / sample_rate).tan();
        let g = (PI * cutoff / sample_rate).tan();

        ComplexSample::new(0.0, w / g)
    }

    fn stage(&self, k: Sample, s: ComplexSample) -> ComplexSample {
        let d = s * s + s * k + 1.0;

        match self.filter_type.shape() {
            Shape::Low => d.inv(),
            Shape::Band => s * k / d,
            Shape::High => s * s / d,
        }
    }

    pub fn at(&self, s: ComplexSample) -> ComplexSample {
        let mut h = self.stage(self.q.recip(), s);

        match self.filter_type.poles() {
            Poles::Two => (),
            Poles::Three => {
                let d = s + 1.0;

                h *= match self.filter_type.shape() {
                    Shape::High => s / d,
                    Shape::Low | Shape::Band => d.inv(),
                };
            }
            Poles::Four => h *= self.stage(BUTTERWORTH_Q.recip(), s),
        }

        h
    }
}
