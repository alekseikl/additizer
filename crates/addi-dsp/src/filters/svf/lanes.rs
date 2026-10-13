//! Per-voice SVF. Each `f32x4` holds four lanes and the integrators stay in that
//! order for the life of the filter. Callers choose the lane map; Additizer uses
//! even-left, even-right, odd-left, odd-right for one voice pair.

use wide::f32x4;

use super::{SvfType, cutoff_to_g_x4};
use crate::Sample;

pub const LANES: usize = 4;

const ONE: f32x4 = f32x4::splat(1.0);
const TWO: f32x4 = f32x4::splat(2.0);
const MIN_GAIN: f32x4 = f32x4::splat(1e-4);

struct LaneParams {
    g: f32x4,
    k: f32x4,
    pre_k: f32x4,
    gain: f32x4,
    input: f32x4,
}

#[derive(Clone, Copy, Default)]
struct Integrator {
    ic1eq: f32x4,
    ic2eq: f32x4,
}

impl Integrator {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, k: f32x4, v0: f32x4) -> (f32x4, f32x4) {
        let a1 = ONE / g.mul_add(g + k, ONE);
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * g.mul_add(v3, self.ic1eq);
        let v2 = g.mul_add(v1, self.ic2eq);

        self.ic1eq = v1.mul_add(TWO, -self.ic1eq);
        self.ic2eq = v2.mul_add(TWO, -self.ic2eq);

        (v1, v2)
    }

    #[inline(always)]
    fn reset(&mut self, mask: f32x4) {
        self.ic1eq = mask.select(f32x4::ZERO, self.ic1eq);
        self.ic2eq = mask.select(f32x4::ZERO, self.ic2eq);
    }
}

#[derive(Clone, Copy, Default)]
struct OnePoleIntegrator {
    s: f32x4,
}

impl OnePoleIntegrator {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, input: f32x4) -> f32x4 {
        let prev = self.s;
        let g = g / (ONE + g);
        let v = (input - prev) * g;
        let lp = v + prev;

        self.s = lp + v;
        lp
    }

    #[inline(always)]
    fn reset(&mut self, mask: f32x4) {
        self.s = mask.select(f32x4::ZERO, self.s);
    }
}

#[derive(Clone, Copy, Default)]
struct LowPass {
    integrator: Integrator,
}

impl LowPass {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, k: f32x4, input: f32x4) -> f32x4 {
        let (_v1, v2) = self.integrator.tick(g, k, input);
        v2
    }

    #[inline(always)]
    fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct HighPass {
    integrator: Integrator,
}

impl HighPass {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, k: f32x4, input: f32x4) -> f32x4 {
        let (v1, v2) = self.integrator.tick(g, k, input);
        k.mul_add(-v1, input) - v2
    }

    #[inline(always)]
    fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct BandPass {
    integrator: Integrator,
}

impl BandPass {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, k: f32x4, input: f32x4) -> f32x4 {
        let (v1, _v2) = self.integrator.tick(g, k, input);
        k * v1
    }

    #[inline(always)]
    fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct Notch {
    integrator: Integrator,
}

impl Notch {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, k: f32x4, input: f32x4) -> f32x4 {
        let (v1, _v2) = self.integrator.tick(g, k, input);
        k.mul_add(-v1, input)
    }

    #[inline(always)]
    fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct Peaking {
    integrator: Integrator,
}

impl Peaking {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, k: f32x4, gain: f32x4, input: f32x4) -> f32x4 {
        let gain = gain.max(MIN_GAIN);
        let k = k / gain.sqrt();
        let (v1, _v2) = self.integrator.tick(g, k, input);
        ((gain - ONE) * k).mul_add(v1, input)
    }

    #[inline(always)]
    fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy)]
struct ShelfCoeffs {
    a: f32x4,
    sqrt_a: f32x4,
    gain: f32x4,
}

impl ShelfCoeffs {
    #[inline(always)]
    fn new(gain: f32x4) -> Self {
        let gain = gain.max(MIN_GAIN);
        let a = gain.sqrt();

        Self {
            a,
            sqrt_a: a.sqrt(),
            gain,
        }
    }

    #[inline(always)]
    fn cascaded(gain: f32x4) -> Self {
        Self::new(gain.max(MIN_GAIN).sqrt())
    }
}

#[derive(Clone, Copy, Default)]
struct LowShelf {
    integrator: Integrator,
}

impl LowShelf {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, k: f32x4, coeffs: ShelfCoeffs, input: f32x4) -> f32x4 {
        let (v1, v2) = self.integrator.tick(g / coeffs.sqrt_a, k, input);
        (coeffs.gain - ONE).mul_add(v2, ((coeffs.a - ONE) * k).mul_add(v1, input))
    }

    #[inline(always)]
    fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct HighShelf {
    integrator: Integrator,
}

impl HighShelf {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, k: f32x4, coeffs: ShelfCoeffs, input: f32x4) -> f32x4 {
        let (v1, v2) = self.integrator.tick(g * coeffs.sqrt_a, k, input);
        (ONE - coeffs.gain).mul_add(
            v2,
            ((coeffs.a - coeffs.gain) * k).mul_add(v1, coeffs.gain * input),
        )
    }

    #[inline(always)]
    fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct OnePoleLowPass {
    integrator: OnePoleIntegrator,
}

impl OnePoleLowPass {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, input: f32x4) -> f32x4 {
        self.integrator.tick(g, input)
    }

    #[inline(always)]
    fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct OnePoleHighPass {
    integrator: OnePoleIntegrator,
}

impl OnePoleHighPass {
    #[inline(always)]
    fn tick(&mut self, g: f32x4, input: f32x4) -> f32x4 {
        input - self.integrator.tick(g, input)
    }

    #[inline(always)]
    fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

trait LaneFilter {
    fn tick(&mut self, p: &LaneParams) -> f32x4;
    fn reset(&mut self, mask: f32x4);
}

#[derive(Clone, Copy, Default)]
struct LowPass12 {
    resonant: LowPass,
}

impl LaneFilter for LowPass12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }

    fn reset(&mut self, mask: f32x4) {
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct LowPass18 {
    resonant: LowPass,
    one_pole: OnePoleLowPass,
}

impl LaneFilter for LowPass18 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        let x = self.resonant.tick(p.g, p.k, p.input * p.gain);
        self.one_pole.tick(p.g, x)
    }

    fn reset(&mut self, mask: f32x4) {
        self.resonant.clear(mask);
        self.one_pole.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct LowPass24 {
    pre_stage: LowPass,
    resonant: LowPass,
}

impl LaneFilter for LowPass24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        let x = self.pre_stage.tick(p.g, p.pre_k, p.input * p.gain);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: f32x4) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct HighPass12 {
    resonant: HighPass,
}

impl LaneFilter for HighPass12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }

    fn reset(&mut self, mask: f32x4) {
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct HighPass18 {
    resonant: HighPass,
    one_pole: OnePoleHighPass,
}

impl LaneFilter for HighPass18 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        let x = self.resonant.tick(p.g, p.k, p.input * p.gain);
        self.one_pole.tick(p.g, x)
    }

    fn reset(&mut self, mask: f32x4) {
        self.resonant.clear(mask);
        self.one_pole.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct HighPass24 {
    pre_stage: HighPass,
    resonant: HighPass,
}

impl LaneFilter for HighPass24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        let x = self.pre_stage.tick(p.g, p.pre_k, p.input * p.gain);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: f32x4) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct BandPass6 {
    resonant: BandPass,
}

impl LaneFilter for BandPass6 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }

    fn reset(&mut self, mask: f32x4) {
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct BandPass12 {
    pre_stage: BandPass,
    resonant: BandPass,
}

impl LaneFilter for BandPass12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        let x = self.pre_stage.tick(p.g, p.pre_k, p.input * p.gain);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: f32x4) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct BandPass18 {
    pre_stages: [BandPass; 2],
    resonant: BandPass,
}

impl LaneFilter for BandPass18 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        let x = self.pre_stages[0].tick(p.g, p.pre_k, p.input * p.gain);
        let x = self.pre_stages[1].tick(p.g, p.pre_k, x);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: f32x4) {
        self.pre_stages[0].clear(mask);
        self.pre_stages[1].clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct BandPass24 {
    pre_stages: [BandPass; 3],
    resonant: BandPass,
}

impl LaneFilter for BandPass24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        let x = self.pre_stages[0].tick(p.g, p.pre_k, p.input * p.gain);
        let x = self.pre_stages[1].tick(p.g, p.pre_k, x);
        let x = self.pre_stages[2].tick(p.g, p.pre_k, x);
        self.resonant.tick(p.g, p.k, x)
    }

    fn reset(&mut self, mask: f32x4) {
        self.pre_stages[0].clear(mask);
        self.pre_stages[1].clear(mask);
        self.pre_stages[2].clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct PeakingFilter {
    section: Peaking,
}

impl LaneFilter for PeakingFilter {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        self.section.tick(p.g, p.k, p.gain, p.input)
    }

    fn reset(&mut self, mask: f32x4) {
        self.section.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct NotchFilter {
    resonant: Notch,
}

impl LaneFilter for NotchFilter {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        self.resonant.tick(p.g, p.k, p.input * p.gain)
    }

    fn reset(&mut self, mask: f32x4) {
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct LowShelf12 {
    section: LowShelf,
}

impl LaneFilter for LowShelf12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        self.section
            .tick(p.g, p.k, ShelfCoeffs::new(p.gain), p.input)
    }

    fn reset(&mut self, mask: f32x4) {
        self.section.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct HighShelf12 {
    section: HighShelf,
}

impl LaneFilter for HighShelf12 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        self.section
            .tick(p.g, p.k, ShelfCoeffs::new(p.gain), p.input)
    }

    fn reset(&mut self, mask: f32x4) {
        self.section.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct LowShelf24 {
    pre_stage: LowShelf,
    resonant: LowShelf,
}

impl LaneFilter for LowShelf24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        let coeffs = ShelfCoeffs::cascaded(p.gain);
        let x = self.pre_stage.tick(p.g, p.pre_k, coeffs, p.input);
        self.resonant.tick(p.g, p.k, coeffs, x)
    }

    fn reset(&mut self, mask: f32x4) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

#[derive(Clone, Copy, Default)]
struct HighShelf24 {
    pre_stage: HighShelf,
    resonant: HighShelf,
}

impl LaneFilter for HighShelf24 {
    #[inline(always)]
    fn tick(&mut self, p: &LaneParams) -> f32x4 {
        let coeffs = ShelfCoeffs::cascaded(p.gain);
        let x = self.pre_stage.tick(p.g, p.pre_k, coeffs, p.input);
        self.resonant.tick(p.g, p.k, coeffs, x)
    }

    fn reset(&mut self, mask: f32x4) {
        self.pre_stage.clear(mask);
        self.resonant.clear(mask);
    }
}

macro_rules! for_filter {
    ($self:expr, $filter:ident => $body:expr) => {
        match $self {
            Filter::LowPass12($filter) => $body,
            Filter::LowPass18($filter) => $body,
            Filter::LowPass24($filter) => $body,
            Filter::HighPass12($filter) => $body,
            Filter::HighPass18($filter) => $body,
            Filter::HighPass24($filter) => $body,
            Filter::BandPass6($filter) => $body,
            Filter::BandPass12($filter) => $body,
            Filter::BandPass18($filter) => $body,
            Filter::BandPass24($filter) => $body,
            Filter::Peaking($filter) => $body,
            Filter::Notch($filter) => $body,
            Filter::LowShelf12($filter) => $body,
            Filter::LowShelf24($filter) => $body,
            Filter::HighShelf12($filter) => $body,
            Filter::HighShelf24($filter) => $body,
        }
    };
}

#[derive(Clone, Copy)]
enum Filter {
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
    Peaking(PeakingFilter),
    Notch(NotchFilter),
    LowShelf12(LowShelf12),
    LowShelf24(LowShelf24),
    HighShelf12(HighShelf12),
    HighShelf24(HighShelf24),
}

/// Four SVF lanes sharing one filter type. Integrator state is `f32x4` and is
/// never unpacked into scalar voices.
#[derive(Clone, Copy)]
pub struct SvfLanes {
    filter: Filter,
}

impl Default for SvfLanes {
    fn default() -> Self {
        Self::new(SvfType::LowPass12)
    }
}

impl SvfLanes {
    pub fn new(filter_type: SvfType) -> Self {
        Self {
            filter: match filter_type {
                SvfType::LowPass12 => Filter::LowPass12(LowPass12::default()),
                SvfType::LowPass18 => Filter::LowPass18(LowPass18::default()),
                SvfType::LowPass24 => Filter::LowPass24(LowPass24::default()),
                SvfType::HighPass12 => Filter::HighPass12(HighPass12::default()),
                SvfType::HighPass18 => Filter::HighPass18(HighPass18::default()),
                SvfType::HighPass24 => Filter::HighPass24(HighPass24::default()),
                SvfType::BandPass6 => Filter::BandPass6(BandPass6::default()),
                SvfType::BandPass12 => Filter::BandPass12(BandPass12::default()),
                SvfType::BandPass18 => Filter::BandPass18(BandPass18::default()),
                SvfType::BandPass24 => Filter::BandPass24(BandPass24::default()),
                SvfType::Peaking => Filter::Peaking(PeakingFilter::default()),
                SvfType::Notch => Filter::Notch(NotchFilter::default()),
                SvfType::LowShelf12 => Filter::LowShelf12(LowShelf12::default()),
                SvfType::LowShelf24 => Filter::LowShelf24(LowShelf24::default()),
                SvfType::HighShelf12 => Filter::HighShelf12(HighShelf12::default()),
                SvfType::HighShelf24 => Filter::HighShelf24(HighShelf24::default()),
            },
        }
    }

    fn filter_type(&self) -> SvfType {
        match self.filter {
            Filter::LowPass12(_) => SvfType::LowPass12,
            Filter::LowPass18(_) => SvfType::LowPass18,
            Filter::LowPass24(_) => SvfType::LowPass24,
            Filter::HighPass12(_) => SvfType::HighPass12,
            Filter::HighPass18(_) => SvfType::HighPass18,
            Filter::HighPass24(_) => SvfType::HighPass24,
            Filter::BandPass6(_) => SvfType::BandPass6,
            Filter::BandPass12(_) => SvfType::BandPass12,
            Filter::BandPass18(_) => SvfType::BandPass18,
            Filter::BandPass24(_) => SvfType::BandPass24,
            Filter::Peaking(_) => SvfType::Peaking,
            Filter::Notch(_) => SvfType::Notch,
            Filter::LowShelf12(_) => SvfType::LowShelf12,
            Filter::LowShelf24(_) => SvfType::LowShelf24,
            Filter::HighShelf12(_) => SvfType::HighShelf12,
            Filter::HighShelf24(_) => SvfType::HighShelf24,
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
            0 => f32x4::new([on, on, 0.0, 0.0]),
            1 => f32x4::new([0.0, 0.0, on, on]),
            _ => panic!("voice slot in a pair is 0 or 1"),
        };

        for_filter!(&mut self.filter, filter => filter.reset(mask));
    }

    /// `store` selects stereo pairs: `0b0011` even, `0b1100` odd, `0b1111` both.
    /// Every lane's integrator still advances.
    #[allow(clippy::too_many_arguments)]
    pub fn process(
        &mut self,
        sample_rate: Sample,
        samples: usize,
        audio: [&[Sample]; LANES],
        cutoff: [&[Sample]; LANES],
        k: [&[Sample]; LANES],
        pre_k: [&[Sample]; LANES],
        gain: [&[Sample]; LANES],
        store: u8,
        output: [&mut [Sample]; LANES],
    ) {
        for_filter!(&mut self.filter, filter => process_samples(
            filter,
            sample_rate,
            samples,
            LaneInputs {
                audio,
                cutoff,
                k,
                pre_k,
                gain,
            },
            store,
            output,
        ));
    }
}

#[inline(always)]
fn take_lanes(lanes: [&[Sample]; LANES], samples: usize) -> [&[Sample]; LANES] {
    lanes.map(|lane| &lane[..samples])
}

#[inline(always)]
fn take_lanes_mut(lanes: [&mut [Sample]; LANES], samples: usize) -> [&mut [Sample]; LANES] {
    lanes.map(|lane| &mut lane[..samples])
}

#[inline(always)]
fn load(lanes: [&[Sample]; LANES], index: usize) -> f32x4 {
    f32x4::new([
        lanes[0][index],
        lanes[1][index],
        lanes[2][index],
        lanes[3][index],
    ])
}

struct LaneInputs<'a> {
    audio: [&'a [Sample]; LANES],
    cutoff: [&'a [Sample]; LANES],
    k: [&'a [Sample]; LANES],
    pre_k: [&'a [Sample]; LANES],
    gain: [&'a [Sample]; LANES],
}

#[inline(always)]
fn step<F: LaneFilter>(
    filter: &mut F,
    freq_mult: Sample,
    index: usize,
    inputs: &LaneInputs<'_>,
) -> f32x4 {
    filter.tick(&LaneParams {
        g: cutoff_to_g_x4(load(inputs.cutoff, index), freq_mult),
        k: load(inputs.k, index),
        pre_k: load(inputs.pre_k, index),
        gain: load(inputs.gain, index),
        input: load(inputs.audio, index),
    })
}

#[allow(clippy::needless_range_loop)]
fn process_samples<F: LaneFilter>(
    filter: &mut F,
    sample_rate: Sample,
    samples: usize,
    inputs: LaneInputs<'_>,
    store: u8,
    output: [&mut [Sample]; LANES],
) {
    let freq_mult = sample_rate.recip();
    let inputs = LaneInputs {
        audio: take_lanes(inputs.audio, samples),
        cutoff: take_lanes(inputs.cutoff, samples),
        k: take_lanes(inputs.k, samples),
        pre_k: take_lanes(inputs.pre_k, samples),
        gain: take_lanes(inputs.gain, samples),
    };
    let [o0, o1, o2, o3] = take_lanes_mut(output, samples);
    let samples = o0.len();

    if store == 0b1111 {
        for index in 0..samples {
            let y = step(filter, freq_mult, index, &inputs).to_array();

            o0[index] = y[0];
            o1[index] = y[1];
            o2[index] = y[2];
            o3[index] = y[3];
        }
    } else if store == 0b0011 {
        for index in 0..samples {
            let y = step(filter, freq_mult, index, &inputs).to_array();

            o0[index] = y[0];
            o1[index] = y[1];
        }
    } else {
        for index in 0..samples {
            let y = step(filter, freq_mult, index, &inputs).to_array();

            o2[index] = y[2];
            o3[index] = y[3];
        }
    }
}
