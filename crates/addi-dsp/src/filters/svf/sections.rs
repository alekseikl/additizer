//! SIMD integrators and the filter sections built on them. Each `f32x4` lane is
//! one voice channel and stays in that slot for the life of the filter.

use wide::f32x4;

const ONE: f32x4 = f32x4::splat(1.0);
const TWO: f32x4 = f32x4::splat(2.0);
const MIN_GAIN: f32x4 = f32x4::splat(1e-4);

#[derive(Clone, Copy, Default)]
pub(super) struct Integrator {
    ic1eq: f32x4,
    ic2eq: f32x4,
}

impl Integrator {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, k: f32x4, v0: f32x4) -> (f32x4, f32x4) {
        let a1 = ONE / g.mul_add(g + k, ONE);
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * g.mul_add(v3, self.ic1eq);
        let v2 = g.mul_add(v1, self.ic2eq);

        self.ic1eq = v1.mul_add(TWO, -self.ic1eq);
        self.ic2eq = v2.mul_add(TWO, -self.ic2eq);

        (v1, v2)
    }

    #[inline(always)]
    pub(super) fn reset(&mut self, mask: f32x4) {
        self.ic1eq = mask.select(f32x4::ZERO, self.ic1eq);
        self.ic2eq = mask.select(f32x4::ZERO, self.ic2eq);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct OnePoleIntegrator {
    s: f32x4,
}

impl OnePoleIntegrator {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, input: f32x4) -> f32x4 {
        let prev = self.s;
        let g = g / (ONE + g);
        let v = (input - prev) * g;
        let lp = v + prev;

        self.s = lp + v;
        lp
    }

    #[inline(always)]
    pub(super) fn reset(&mut self, mask: f32x4) {
        self.s = mask.select(f32x4::ZERO, self.s);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct LowPass {
    integrator: Integrator,
}

impl LowPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, k: f32x4, input: f32x4) -> f32x4 {
        let (_v1, v2) = self.integrator.tick(g, k, input);
        v2
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct HighPass {
    integrator: Integrator,
}

impl HighPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, k: f32x4, input: f32x4) -> f32x4 {
        let (v1, v2) = self.integrator.tick(g, k, input);
        k.mul_add(-v1, input) - v2
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct BandPass {
    integrator: Integrator,
}

impl BandPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, k: f32x4, input: f32x4) -> f32x4 {
        let (v1, _v2) = self.integrator.tick(g, k, input);
        k * v1
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Notch {
    integrator: Integrator,
}

impl Notch {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, k: f32x4, input: f32x4) -> f32x4 {
        let (v1, _v2) = self.integrator.tick(g, k, input);
        k.mul_add(-v1, input)
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Peaking {
    integrator: Integrator,
}

impl Peaking {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, k: f32x4, gain: f32x4, input: f32x4) -> f32x4 {
        let gain = gain.max(MIN_GAIN);
        let k = k / gain.sqrt();
        let (v1, _v2) = self.integrator.tick(g, k, input);
        ((gain - ONE) * k).mul_add(v1, input)
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy)]
pub(super) struct ShelfCoeffs {
    a: f32x4,
    sqrt_a: f32x4,
    gain: f32x4,
}

impl ShelfCoeffs {
    #[inline(always)]
    pub(super) fn new(gain: f32x4) -> Self {
        let gain = gain.max(MIN_GAIN);
        let a = gain.sqrt();

        Self {
            a,
            sqrt_a: a.sqrt(),
            gain,
        }
    }

    #[inline(always)]
    pub(super) fn cascaded(gain: f32x4) -> Self {
        Self::new(gain.max(MIN_GAIN).sqrt())
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct LowShelf {
    integrator: Integrator,
}

impl LowShelf {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, k: f32x4, coeffs: ShelfCoeffs, input: f32x4) -> f32x4 {
        let (v1, v2) = self.integrator.tick(g / coeffs.sqrt_a, k, input);
        (coeffs.gain - ONE).mul_add(v2, ((coeffs.a - ONE) * k).mul_add(v1, input))
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct HighShelf {
    integrator: Integrator,
}

impl HighShelf {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, k: f32x4, coeffs: ShelfCoeffs, input: f32x4) -> f32x4 {
        let (v1, v2) = self.integrator.tick(g * coeffs.sqrt_a, k, input);
        (ONE - coeffs.gain).mul_add(
            v2,
            ((coeffs.a - coeffs.gain) * k).mul_add(v1, coeffs.gain * input),
        )
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct OnePoleLowPass {
    integrator: OnePoleIntegrator,
}

impl OnePoleLowPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, input: f32x4) -> f32x4 {
        self.integrator.tick(g, input)
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct OnePoleHighPass {
    integrator: OnePoleIntegrator,
}

impl OnePoleHighPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: f32x4, input: f32x4) -> f32x4 {
        input - self.integrator.tick(g, input)
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: f32x4) {
        self.integrator.reset(mask);
    }
}
