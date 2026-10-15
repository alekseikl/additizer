use std::array;

use serde::{Deserialize, Serialize};

use crate::{
    ComplexSample, SIMD_LANES, Smp, WideSmp,
    filters::control::ZERO_RESONANCE_Q,
    math::{db_to_gain, db_to_gain_fast},
};

mod sections;
mod simd;

use sections::{BandPass, HighPass, HighShelf, LowPass, LowShelf, OnePoleHighPass, OnePoleLowPass};
use simd::WideComplex;

pub trait FilterImpl: Clone + Copy + 'static {
    fn new(gain: Smp, cutoff_freq: Smp, q: Smp, pre_q: Smp) -> Self;
}

trait FilterLanes: FilterImpl {
    fn at(&self, freq: WideSmp) -> WideComplex;
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
    fn new(gain: Smp, cutoff: Smp, q: Smp, _pre_q: Smp) -> Self {
        Self {
            resonant: LowPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for LowPass12 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.resonant.at(freq)
    }
}

impl FilterImpl for LowPass18 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, _pre_q: Smp) -> Self {
        Self {
            pre_stage: OnePoleLowPass::new(cutoff),
            resonant: LowPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for LowPass18 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.pre_stage.at(freq) * self.resonant.at(freq)
    }
}

impl FilterImpl for LowPass24 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, pre_q: Smp) -> Self {
        Self {
            pre_stage: LowPass::new(1.0, cutoff, pre_q),
            resonant: LowPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for LowPass24 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.pre_stage.at(freq) * self.resonant.at(freq)
    }
}

impl FilterImpl for HighPass12 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, _pre_q: Smp) -> Self {
        Self {
            resonant: HighPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for HighPass12 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.resonant.at(freq)
    }
}

impl FilterImpl for HighPass18 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, _pre_q: Smp) -> Self {
        Self {
            pre_stage: OnePoleHighPass::new(cutoff),
            resonant: HighPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for HighPass18 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.pre_stage.at(freq) * self.resonant.at(freq)
    }
}

impl FilterImpl for HighPass24 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, pre_q: Smp) -> Self {
        Self {
            pre_stage: HighPass::new(1.0, cutoff, pre_q),
            resonant: HighPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for HighPass24 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.pre_stage.at(freq) * self.resonant.at(freq)
    }
}

impl FilterImpl for BandPass6 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, _pre_q: Smp) -> Self {
        Self {
            resonant: BandPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for BandPass6 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.resonant.at(freq)
    }
}

impl FilterImpl for BandPass12 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, pre_q: Smp) -> Self {
        Self {
            pre_stage: BandPass::new(1.0, cutoff, pre_q),
            resonant: BandPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for BandPass12 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.pre_stage.at(freq) * self.resonant.at(freq)
    }
}

impl FilterImpl for BandPass18 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, pre_q: Smp) -> Self {
        Self {
            pre_stage: BandPass::new(1.0, cutoff, pre_q),
            resonant: BandPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for BandPass18 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        let pre_stage = self.pre_stage.at(freq);

        pre_stage * pre_stage * self.resonant.at(freq)
    }
}

impl FilterImpl for BandPass24 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, pre_q: Smp) -> Self {
        Self {
            pre_stage: BandPass::new(1.0, cutoff, pre_q),
            resonant: BandPass::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for BandPass24 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        let pre_stage = self.pre_stage.at(freq);
        let pre_stage_sq = pre_stage * pre_stage;

        pre_stage_sq * pre_stage * self.resonant.at(freq)
    }
}

impl FilterImpl for Peaking {
    fn new(gain: Smp, cutoff: Smp, q: Smp, _pre_q: Smp) -> Self {
        Self {
            section: sections::Peaking::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for Peaking {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.section.at(freq)
    }
}

impl FilterImpl for Notch {
    fn new(gain: Smp, cutoff: Smp, q: Smp, _pre_q: Smp) -> Self {
        Self {
            resonant: sections::Notch::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for Notch {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.resonant.at(freq)
    }
}

impl FilterImpl for LowShelf12 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, _pre_q: Smp) -> Self {
        Self {
            section: LowShelf::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for LowShelf12 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.section.at(freq)
    }
}

impl FilterImpl for HighShelf12 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, _pre_q: Smp) -> Self {
        Self {
            section: HighShelf::new(gain, cutoff, q),
        }
    }
}

impl FilterLanes for HighShelf12 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.section.at(freq)
    }
}

impl FilterImpl for LowShelf24 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, pre_q: Smp) -> Self {
        let g12 = gain.max(0.0).sqrt();

        Self {
            pre_stage: LowShelf::new(g12, cutoff, pre_q),
            resonant: LowShelf::new(g12, cutoff, q),
        }
    }
}

impl FilterLanes for LowShelf24 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.pre_stage.at(freq) * self.resonant.at(freq)
    }
}

impl FilterImpl for HighShelf24 {
    fn new(gain: Smp, cutoff: Smp, q: Smp, pre_q: Smp) -> Self {
        let g12 = gain.max(0.0).sqrt();

        Self {
            pre_stage: HighShelf::new(g12, cutoff, pre_q),
            resonant: HighShelf::new(g12, cutoff, q),
        }
    }
}

impl FilterLanes for HighShelf24 {
    #[inline(always)]
    fn at(&self, freq: WideSmp) -> WideComplex {
        self.pre_stage.at(freq) * self.resonant.at(freq)
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
    pub drive: Smp,
    pub cutoff: Smp, // Octaves of the note fundamental (harmonic space).
    pub q: Smp,
    /// Pre-stage Q in a cascade.
    pub pre_q: Smp,
    pub q_cutoff: Smp,  // Octaves. At and above this, Q is not reduced.
    pub q_rolloff: Smp, // dB per octave below q_cutoff.
    pub linear_phase: bool,
}

pub struct SpectralFilter {
    filter_type: FilterType,
    gain: Smp,
    cutoff_freq: Smp,
    q: Smp,
    pre_q: Smp,
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

    fn q_from_params(params: &FilterParams) -> Smp {
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

    pub fn response_at_freqs(&self, freqs: &[Smp], out: &mut [ComplexSample]) {
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

    fn response_freqs_impl<T: FilterLanes>(&self, freqs: &[Smp], out: &mut [ComplexSample]) {
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
            apply::<T, true>(&filter_impl, input, output, len);
        } else {
            apply::<T, false>(&filter_impl, input, output, len);
        }
    }

    fn apply_in_place_impl<T: FilterLanes>(&self, samples: &mut [ComplexSample]) {
        let filter_impl = T::new(self.gain, self.cutoff_freq, self.q, self.pre_q);

        if self.linear_phase {
            apply_in_place::<T, true>(&filter_impl, samples);
        } else {
            apply_in_place::<T, false>(&filter_impl, samples);
        }
    }
}

const DC_OFFSET: usize = 1;

/// Bin frequencies for the `SIMD_LANES` bins starting at `start`.
#[inline(always)]
fn bin_freqs(start: usize) -> WideSmp {
    WideSmp::new(array::from_fn(|lane| (start + lane) as Smp))
}

#[inline(always)]
fn write_response_lanes<const LINEAR: bool>(response: WideComplex, out: &mut [ComplexSample]) {
    if LINEAR {
        for (sample, mag) in out.iter_mut().zip(response.norm().to_array()) {
            *sample = ComplexSample::new(mag, 0.0);
        }
    } else {
        let re = response.re.to_array();
        let im = response.im.to_array();
        for (sample, (re, im)) in out.iter_mut().zip(re.into_iter().zip(im)) {
            *sample = ComplexSample::new(re, im);
        }
    }
}

fn write_responses<T: FilterLanes, const LINEAR: bool>(
    filter: &T,
    freqs: &[Smp],
    out: &mut [ComplexSample],
) {
    let n = freqs.len().min(out.len());

    for (freq_chunk, out_chunk) in freqs[..n]
        .chunks(SIMD_LANES)
        .zip(out[..n].chunks_mut(SIMD_LANES))
    {
        write_response_lanes::<LINEAR>(filter.at(WideSmp::from(freq_chunk)), out_chunk);
    }
}

#[inline(always)]
fn store_lanes<const LINEAR: bool>(
    response: WideComplex,
    input: &[ComplexSample],
    output: &mut [ComplexSample],
) {
    if LINEAR {
        for ((out, sample), mag) in output.iter_mut().zip(input).zip(response.norm().to_array()) {
            *out = *sample * mag;
        }
    } else {
        let re = response.re.to_array();
        let im = response.im.to_array();
        for ((out, sample), (re, im)) in output.iter_mut().zip(input).zip(re.into_iter().zip(im)) {
            *out = *sample * ComplexSample::new(re, im);
        }
    }
}

fn apply<T: FilterLanes, const LINEAR: bool>(
    filter: &T,
    input: &[ComplexSample],
    output: &mut [ComplexSample],
    len: usize,
) {
    // Skip DC. Bin indices are the frequencies.
    if len <= DC_OFFSET {
        return;
    }

    let input = &input[DC_OFFSET..len];
    let output = &mut output[DC_OFFSET..len];
    let (input_chunks, input_tail) = input.as_chunks::<SIMD_LANES>();
    let (output_chunks, output_tail) = output.as_chunks_mut::<SIMD_LANES>();

    for (start, (input_chunk, output_chunk)) in (DC_OFFSET..)
        .step_by(SIMD_LANES)
        .zip(input_chunks.iter().zip(output_chunks))
    {
        store_lanes::<LINEAR>(filter.at(bin_freqs(start)), input_chunk, output_chunk);
    }

    if !input_tail.is_empty() {
        let start = DC_OFFSET + input_chunks.len() * SIMD_LANES;
        store_lanes::<LINEAR>(filter.at(bin_freqs(start)), input_tail, output_tail);
    }
}

#[inline(always)]
fn apply_lanes_in_place<const LINEAR: bool>(response: WideComplex, samples: &mut [ComplexSample]) {
    if LINEAR {
        for (sample, mag) in samples.iter_mut().zip(response.norm().to_array()) {
            *sample *= mag;
        }
    } else {
        let re = response.re.to_array();
        let im = response.im.to_array();
        for (sample, (re, im)) in samples.iter_mut().zip(re.into_iter().zip(im)) {
            *sample *= ComplexSample::new(re, im);
        }
    }
}

fn apply_in_place<T: FilterLanes, const LINEAR: bool>(filter: &T, samples: &mut [ComplexSample]) {
    // Skip DC. Bin indices are the frequencies.
    if samples.len() <= DC_OFFSET {
        return;
    }

    let samples = &mut samples[DC_OFFSET..];
    let (chunks, tail) = samples.as_chunks_mut::<SIMD_LANES>();

    for (start, chunk) in (DC_OFFSET..).step_by(SIMD_LANES).zip(&mut *chunks) {
        apply_lanes_in_place::<LINEAR>(filter.at(bin_freqs(start)), chunk);
    }

    if !tail.is_empty() {
        let start = DC_OFFSET + chunks.len() * SIMD_LANES;
        apply_lanes_in_place::<LINEAR>(filter.at(bin_freqs(start)), tail);
    }
}

#[cfg(test)]
mod tests;
