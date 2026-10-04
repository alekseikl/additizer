use serde::{Deserialize, Serialize};
use wide::f32x4;

use crate::{
    ComplexSample, Sample,
    filters::control::ZERO_RESONANCE_Q,
    units::{db_to_gain, db_to_gain_fast},
};

mod sections;
mod simd;

use sections::{BandPass, HighPass, HighShelf, LowPass, LowShelf, OnePoleHighPass, OnePoleLowPass};
use simd::ComplexX4;

#[cfg(test)]
use sections::TAU;

pub trait FilterImpl: Clone + Copy + 'static {
    fn new(gain: Sample, cutoff_freq: Sample, q: Sample, pre_q: Sample) -> Self;
}

trait FilterLanes: FilterImpl {
    fn at_x4(&self, freq: f32x4) -> ComplexX4;

    /// Lane 0 of [`Self::at_x4`] at a single frequency.
    #[inline]
    fn at(&self, freq: Sample) -> ComplexSample {
        lane0(self.at_x4(f32x4::splat(freq)))
    }
}

#[derive(Clone, Copy)]
pub struct LowPass12 {
    resonant: LowPass,
}

#[derive(Clone, Copy)]
pub struct LowPass18 {
    pre_stage: OnePoleLowPass,
    resonant: LowPass,
}

#[derive(Clone, Copy)]
pub struct LowPass24 {
    pre_stage: LowPass,
    resonant: LowPass,
}

#[derive(Clone, Copy)]
pub struct HighPass12 {
    resonant: HighPass,
}

#[derive(Clone, Copy)]
pub struct HighPass18 {
    pre_stage: OnePoleHighPass,
    resonant: HighPass,
}

#[derive(Clone, Copy)]
pub struct HighPass24 {
    pre_stage: HighPass,
    resonant: HighPass,
}

#[derive(Clone, Copy)]
pub struct BandPass6 {
    resonant: BandPass,
}

#[derive(Clone, Copy)]
pub struct BandPass12 {
    pre_stage: BandPass,
    resonant: BandPass,
}

#[derive(Clone, Copy)]
pub struct BandPass18 {
    pre_stage: BandPass,
    resonant: BandPass,
}

#[derive(Clone, Copy)]
pub struct BandPass24 {
    pre_stage: BandPass,
    resonant: BandPass,
}

#[derive(Clone, Copy)]
pub struct Peaking {
    section: sections::Peaking,
}

#[derive(Clone, Copy)]
pub struct Notch {
    resonant: sections::Notch,
}

#[derive(Clone, Copy)]
pub struct LowShelf12 {
    section: LowShelf,
}

#[derive(Clone, Copy)]
pub struct HighShelf12 {
    section: HighShelf,
}

#[derive(Clone, Copy)]
pub struct LowShelf24 {
    pre_stage: LowShelf,
    resonant: LowShelf,
}

#[derive(Clone, Copy)]
pub struct HighShelf24 {
    pre_stage: HighShelf,
    resonant: HighShelf,
}

impl FilterImpl for LowPass12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, _pre_q: Sample) -> Self {
        Self {
            resonant: LowPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for LowPass12 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.resonant.at_x4(freq)
    }
}

impl FilterImpl for LowPass18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, _pre_q: Sample) -> Self {
        Self {
            pre_stage: OnePoleLowPass::new(cutoff),
            resonant: LowPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for LowPass18 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.pre_stage.at_x4(freq) * self.resonant.at_x4(freq)
    }
}

impl FilterImpl for LowPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, pre_q: Sample) -> Self {
        Self {
            pre_stage: LowPass::new(1.0, cutoff, pre_q),
            resonant: LowPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for LowPass24 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.pre_stage.at_x4(freq) * self.resonant.at_x4(freq)
    }
}

impl FilterImpl for HighPass12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, _pre_q: Sample) -> Self {
        Self {
            resonant: HighPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for HighPass12 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.resonant.at_x4(freq)
    }
}

impl FilterImpl for HighPass18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, _pre_q: Sample) -> Self {
        Self {
            pre_stage: OnePoleHighPass::new(cutoff),
            resonant: HighPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for HighPass18 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.pre_stage.at_x4(freq) * self.resonant.at_x4(freq)
    }
}

impl FilterImpl for HighPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, pre_q: Sample) -> Self {
        Self {
            pre_stage: HighPass::new(1.0, cutoff, pre_q),
            resonant: HighPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for HighPass24 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.pre_stage.at_x4(freq) * self.resonant.at_x4(freq)
    }
}

impl FilterImpl for BandPass6 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, _pre_q: Sample) -> Self {
        Self {
            resonant: BandPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for BandPass6 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.resonant.at_x4(freq)
    }
}

impl FilterImpl for BandPass12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, pre_q: Sample) -> Self {
        Self {
            pre_stage: BandPass::new(1.0, cutoff, pre_q),
            resonant: BandPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for BandPass12 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.pre_stage.at_x4(freq) * self.resonant.at_x4(freq)
    }
}

impl FilterImpl for BandPass18 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, pre_q: Sample) -> Self {
        Self {
            pre_stage: BandPass::new(1.0, cutoff, pre_q),
            resonant: BandPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for BandPass18 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        let pre_stage = self.pre_stage.at_x4(freq);

        pre_stage * pre_stage * self.resonant.at_x4(freq)
    }
}

impl FilterImpl for BandPass24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, pre_q: Sample) -> Self {
        Self {
            pre_stage: BandPass::new(1.0, cutoff, pre_q),
            resonant: BandPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for BandPass24 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        let pre_stage = self.pre_stage.at_x4(freq);
        let pre_stage_sq = pre_stage * pre_stage;

        pre_stage_sq * pre_stage * self.resonant.at_x4(freq)
    }
}

impl FilterImpl for Peaking {
    fn new(gain: Sample, cutoff: Sample, q: Sample, _pre_q: Sample) -> Self {
        Self {
            section: sections::Peaking::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for Peaking {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.section.at_x4(freq)
    }
}

impl FilterImpl for Notch {
    fn new(gain: Sample, cutoff: Sample, q: Sample, _pre_q: Sample) -> Self {
        Self {
            resonant: sections::Notch::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for Notch {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.resonant.at_x4(freq)
    }
}

impl FilterImpl for LowShelf12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, _pre_q: Sample) -> Self {
        Self {
            section: LowShelf::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for LowShelf12 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.section.at_x4(freq)
    }
}

impl FilterImpl for HighShelf12 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, _pre_q: Sample) -> Self {
        Self {
            section: HighShelf::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for HighShelf12 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.section.at_x4(freq)
    }
}

impl FilterImpl for LowShelf24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, pre_q: Sample) -> Self {
        let g12 = gain.max(0.0).sqrt();

        Self {
            pre_stage: LowShelf::new(g12, cutoff, pre_q),
            resonant: LowShelf::new(g12, cutoff, q),
        }
    }
}

impl FilterLanes for LowShelf24 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.pre_stage.at_x4(freq) * self.resonant.at_x4(freq)
    }
}

impl FilterImpl for HighShelf24 {
    fn new(gain: Sample, cutoff: Sample, q: Sample, pre_q: Sample) -> Self {
        let g12 = gain.max(0.0).sqrt();

        Self {
            pre_stage: HighShelf::new(g12, cutoff, pre_q),
            resonant: HighShelf::new(g12, cutoff, q),
        }
    }
}

impl FilterLanes for HighShelf24 {
    #[inline(always)]
    fn at_x4(&self, freq: f32x4) -> ComplexX4 {
        self.pre_stage.at_x4(freq) * self.resonant.at_x4(freq)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FilterType {
    #[default]
    LowPass12,
    LowPass18,
    LowPass24,
    HighPass12,
    HighPass18,
    HighPass24,
    #[serde(alias = "BandPass")]
    BandPass6,
    BandPass12,
    BandPass18,
    BandPass24,
    Peaking,
    Notch,
    #[serde(alias = "LowShelf")]
    LowShelf12,
    LowShelf24,
    #[serde(alias = "HighShelf")]
    HighShelf12,
    HighShelf24,
}

impl FilterType {
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

#[derive(Clone, Copy)]
pub struct FilterParams {
    pub drive: Sample,
    pub cutoff: Sample, // Octaves of the note fundamental (harmonic space).
    pub q: Sample,
    /// Pre-stage Q in a cascade.
    pub pre_q: Sample,
    pub q_cutoff: Sample,  // Octaves. At and above this, Q is not reduced.
    pub q_rolloff: Sample, // dB per octave below q_cutoff.
    pub linear_phase: bool,
}

pub struct SpectralFilter {
    filter_type: FilterType,
    gain: Sample,
    cutoff_freq: Sample,
    q: Sample,
    pre_q: Sample,
    linear_phase: bool,
}

impl SpectralFilter {
    pub fn new(filter_type: FilterType, params: FilterParams) -> Self {
        Self {
            filter_type,
            gain: db_to_gain_fast(params.drive),
            cutoff_freq: params.cutoff.exp2(),
            q: Self::q_from_params(&params),
            pre_q: params.pre_q,
            linear_phase: params.linear_phase,
        }
    }

    fn q_from_params(params: &FilterParams) -> Sample {
        let q = params.q;
        let excess = q - ZERO_RESONANCE_Q;

        if excess <= 0.0 || params.cutoff >= params.q_cutoff {
            return q;
        }

        let octaves_below = params.q_cutoff - params.cutoff;
        let scale = db_to_gain(-params.q_rolloff * octaves_below);

        ZERO_RESONANCE_Q + excess * scale
    }

    pub fn apply_response(&self, input: &[ComplexSample], output: &mut [ComplexSample]) {
        match self.filter_type {
            FilterType::LowPass12 => self.apply_impl::<LowPass12>(input, output),
            FilterType::LowPass18 => self.apply_impl::<LowPass18>(input, output),
            FilterType::LowPass24 => self.apply_impl::<LowPass24>(input, output),
            FilterType::LowShelf12 => self.apply_impl::<LowShelf12>(input, output),
            FilterType::LowShelf24 => self.apply_impl::<LowShelf24>(input, output),
            FilterType::HighPass12 => self.apply_impl::<HighPass12>(input, output),
            FilterType::HighPass18 => self.apply_impl::<HighPass18>(input, output),
            FilterType::HighPass24 => self.apply_impl::<HighPass24>(input, output),
            FilterType::HighShelf12 => self.apply_impl::<HighShelf12>(input, output),
            FilterType::HighShelf24 => self.apply_impl::<HighShelf24>(input, output),
            FilterType::BandPass6 => self.apply_impl::<BandPass6>(input, output),
            FilterType::BandPass12 => self.apply_impl::<BandPass12>(input, output),
            FilterType::BandPass18 => self.apply_impl::<BandPass18>(input, output),
            FilterType::BandPass24 => self.apply_impl::<BandPass24>(input, output),
            FilterType::Peaking => self.apply_impl::<Peaking>(input, output),
            FilterType::Notch => self.apply_impl::<Notch>(input, output),
        }
    }

    pub fn apply_response_in_place(&self, samples: &mut [ComplexSample]) {
        match self.filter_type {
            FilterType::LowPass12 => self.apply_in_place_impl::<LowPass12>(samples),
            FilterType::LowPass18 => self.apply_in_place_impl::<LowPass18>(samples),
            FilterType::LowPass24 => self.apply_in_place_impl::<LowPass24>(samples),
            FilterType::LowShelf12 => self.apply_in_place_impl::<LowShelf12>(samples),
            FilterType::LowShelf24 => self.apply_in_place_impl::<LowShelf24>(samples),
            FilterType::HighPass12 => self.apply_in_place_impl::<HighPass12>(samples),
            FilterType::HighPass18 => self.apply_in_place_impl::<HighPass18>(samples),
            FilterType::HighPass24 => self.apply_in_place_impl::<HighPass24>(samples),
            FilterType::HighShelf12 => self.apply_in_place_impl::<HighShelf12>(samples),
            FilterType::HighShelf24 => self.apply_in_place_impl::<HighShelf24>(samples),
            FilterType::BandPass6 => self.apply_in_place_impl::<BandPass6>(samples),
            FilterType::BandPass12 => self.apply_in_place_impl::<BandPass12>(samples),
            FilterType::BandPass18 => self.apply_in_place_impl::<BandPass18>(samples),
            FilterType::BandPass24 => self.apply_in_place_impl::<BandPass24>(samples),
            FilterType::Peaking => self.apply_in_place_impl::<Peaking>(samples),
            FilterType::Notch => self.apply_in_place_impl::<Notch>(samples),
        }
    }

    pub fn response_at_freqs(&self, freqs: &[Sample], out: &mut [ComplexSample]) {
        match self.filter_type {
            FilterType::LowPass12 => self.response_freqs_impl::<LowPass12>(freqs, out),
            FilterType::LowPass18 => self.response_freqs_impl::<LowPass18>(freqs, out),
            FilterType::LowPass24 => self.response_freqs_impl::<LowPass24>(freqs, out),
            FilterType::LowShelf12 => self.response_freqs_impl::<LowShelf12>(freqs, out),
            FilterType::LowShelf24 => self.response_freqs_impl::<LowShelf24>(freqs, out),
            FilterType::HighPass12 => self.response_freqs_impl::<HighPass12>(freqs, out),
            FilterType::HighPass18 => self.response_freqs_impl::<HighPass18>(freqs, out),
            FilterType::HighPass24 => self.response_freqs_impl::<HighPass24>(freqs, out),
            FilterType::HighShelf12 => self.response_freqs_impl::<HighShelf12>(freqs, out),
            FilterType::HighShelf24 => self.response_freqs_impl::<HighShelf24>(freqs, out),
            FilterType::BandPass6 => self.response_freqs_impl::<BandPass6>(freqs, out),
            FilterType::BandPass12 => self.response_freqs_impl::<BandPass12>(freqs, out),
            FilterType::BandPass18 => self.response_freqs_impl::<BandPass18>(freqs, out),
            FilterType::BandPass24 => self.response_freqs_impl::<BandPass24>(freqs, out),
            FilterType::Peaking => self.response_freqs_impl::<Peaking>(freqs, out),
            FilterType::Notch => self.response_freqs_impl::<Notch>(freqs, out),
        }
    }

    fn response_freqs_impl<T: FilterLanes>(&self, freqs: &[Sample], out: &mut [ComplexSample]) {
        let filter_impl = T::new(self.gain, self.cutoff_freq, self.q, self.pre_q);

        if self.linear_phase {
            write_responses::<T, true>(&filter_impl, freqs, out);
        } else {
            write_responses::<T, false>(&filter_impl, freqs, out);
        }
    }

    fn apply_impl<T: FilterLanes>(&self, input: &[ComplexSample], output: &mut [ComplexSample]) {
        let filter_impl = T::new(self.gain, self.cutoff_freq, self.q, self.pre_q);
        let len = input.len().min(output.len());

        if self.linear_phase {
            apply_lanes::<T, true>(&filter_impl, input, output, len);
        } else {
            apply_lanes::<T, false>(&filter_impl, input, output, len);
        }
    }

    fn apply_in_place_impl<T: FilterLanes>(&self, samples: &mut [ComplexSample]) {
        let filter_impl = T::new(self.gain, self.cutoff_freq, self.q, self.pre_q);

        if self.linear_phase {
            apply_lanes_in_place::<T, true>(&filter_impl, samples);
        } else {
            apply_lanes_in_place::<T, false>(&filter_impl, samples);
        }
    }
}

const LANES: usize = 4;
const DC_OFFSET: usize = 1;

#[inline]
fn lane0(response: ComplexX4) -> ComplexSample {
    let re = response.re.to_array();
    let im = response.im.to_array();
    ComplexSample::new(re[0], im[0])
}

/// Up to four frequencies. Lanes past the slice stay zero and are not stored.
#[inline(always)]
fn load_freq_lanes(freqs: &[Sample]) -> f32x4 {
    let mut lanes = [0.0; LANES];
    let n = freqs.len().min(LANES);
    lanes[..n].copy_from_slice(&freqs[..n]);
    f32x4::new(lanes)
}

#[inline(always)]
fn write_response_lanes<const LINEAR: bool>(
    response: ComplexX4,
    out: &mut [ComplexSample],
    i: usize,
    n: usize,
) {
    if LINEAR {
        let mag = response.norm().to_array();
        for k in 0..n {
            out[i + k] = ComplexSample::new(mag[k], 0.0);
        }
    } else {
        let re = response.re.to_array();
        let im = response.im.to_array();
        for k in 0..n {
            out[i + k] = ComplexSample::new(re[k], im[k]);
        }
    }
}

fn write_responses<T: FilterLanes, const LINEAR: bool>(
    filter: &T,
    freqs: &[Sample],
    out: &mut [ComplexSample],
) {
    let n = freqs.len().min(out.len());
    let mut i = 0;

    while i + LANES <= n {
        write_response_lanes::<LINEAR>(
            filter.at_x4(f32x4::new([
                freqs[i],
                freqs[i + 1],
                freqs[i + 2],
                freqs[i + 3],
            ])),
            out,
            i,
            LANES,
        );
        i += LANES;
    }

    if i < n {
        write_response_lanes::<LINEAR>(filter.at_x4(load_freq_lanes(&freqs[i..])), out, i, n - i);
    }
}

#[inline(always)]
fn store_lanes<const LINEAR: bool>(
    response: ComplexX4,
    input: &[ComplexSample],
    output: &mut [ComplexSample],
    i: usize,
    n: usize,
) {
    if LINEAR {
        let mag = response.norm().to_array();
        for k in 0..n {
            output[i + k] = input[i + k] * mag[k];
        }
    } else {
        let re = response.re.to_array();
        let im = response.im.to_array();
        for k in 0..n {
            output[i + k] = input[i + k] * ComplexSample::new(re[k], im[k]);
        }
    }
}

#[inline(always)]
fn scale_lanes_in_place<const LINEAR: bool>(
    response: ComplexX4,
    samples: &mut [ComplexSample],
    i: usize,
    n: usize,
) {
    if LINEAR {
        let mag = response.norm().to_array();
        for k in 0..n {
            samples[i + k] *= mag[k];
        }
    } else {
        let re = response.re.to_array();
        let im = response.im.to_array();
        for k in 0..n {
            samples[i + k] *= ComplexSample::new(re[k], im[k]);
        }
    }
}

/// Frequencies for one chunk. Lanes past `n` are zero and must not be stored.
#[inline(always)]
fn chunk_freq(freq: f32x4, n: usize) -> f32x4 {
    let mut lanes = freq.to_array();
    for lane in lanes.iter_mut().skip(n) {
        *lane = 0.0;
    }
    f32x4::new(lanes)
}

fn apply_lanes<T: FilterLanes, const LINEAR: bool>(
    filter: &T,
    input: &[ComplexSample],
    output: &mut [ComplexSample],
    len: usize,
) {
    // Skip DC. Bin indices are the frequencies.
    let mut i = DC_OFFSET;
    let mut freq = f32x4::new([1.0, 2.0, 3.0, 4.0]);
    let step = f32x4::splat(LANES as Sample);

    while i + LANES <= len {
        store_lanes::<LINEAR>(filter.at_x4(freq), input, output, i, LANES);
        freq += step;
        i += LANES;
    }

    if i < len {
        let n = len - i;
        store_lanes::<LINEAR>(filter.at_x4(chunk_freq(freq, n)), input, output, i, n);
    }
}

fn apply_lanes_in_place<T: FilterLanes, const LINEAR: bool>(
    filter: &T,
    samples: &mut [ComplexSample],
) {
    let len = samples.len();
    let mut i = DC_OFFSET;
    let mut freq = f32x4::new([1.0, 2.0, 3.0, 4.0]);
    let step = f32x4::splat(LANES as Sample);

    while i + LANES <= len {
        scale_lanes_in_place::<LINEAR>(filter.at_x4(freq), samples, i, LANES);
        freq += step;
        i += LANES;
    }

    if i < len {
        let n = len - i;
        scale_lanes_in_place::<LINEAR>(filter.at_x4(chunk_freq(freq, n)), samples, i, n);
    }
}

#[cfg(test)]
mod tests;
