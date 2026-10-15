//! SIMD integrators and the filter sections built on them. Each [`WideSmp`] lane is
//! one voice channel and stays in that slot for the life of the filter.

use crate::WideSmp;

const ONE: WideSmp = WideSmp::splat(1.0);
const TWO: WideSmp = WideSmp::splat(2.0);
const MIN_GAIN: WideSmp = WideSmp::splat(1e-4);

#[derive(Clone, Copy, Default)]
pub(super) struct Integrator {
    ic1eq: WideSmp,
    ic2eq: WideSmp,
}

impl Integrator {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, k: WideSmp, v0: WideSmp) -> (WideSmp, WideSmp) {
        let a1 = ONE / g.mul_add(g + k, ONE);
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * g.mul_add(v3, self.ic1eq);
        let v2 = g.mul_add(v1, self.ic2eq);

        self.ic1eq = v1.mul_add(TWO, -self.ic1eq);
        self.ic2eq = v2.mul_add(TWO, -self.ic2eq);

        (v1, v2)
    }

    #[inline(always)]
    pub(super) fn reset(&mut self, mask: WideSmp) {
        self.ic1eq = mask.select(WideSmp::ZERO, self.ic1eq);
        self.ic2eq = mask.select(WideSmp::ZERO, self.ic2eq);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct OnePoleIntegrator {
    s: WideSmp,
}

impl OnePoleIntegrator {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, input: WideSmp) -> WideSmp {
        let prev = self.s;
        let g = g / (ONE + g);
        let v = (input - prev) * g;
        let lp = v + prev;

        self.s = lp + v;
        lp
    }

    #[inline(always)]
    pub(super) fn reset(&mut self, mask: WideSmp) {
        self.s = mask.select(WideSmp::ZERO, self.s);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct LowPass {
    integrator: Integrator,
}

impl LowPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, k: WideSmp, input: WideSmp) -> WideSmp {
        let (_v1, v2) = self.integrator.tick(g, k, input);
        v2
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: WideSmp) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct HighPass {
    integrator: Integrator,
}

impl HighPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, k: WideSmp, input: WideSmp) -> WideSmp {
        let (v1, v2) = self.integrator.tick(g, k, input);
        k.mul_add(-v1, input) - v2
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: WideSmp) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct BandPass {
    integrator: Integrator,
}

impl BandPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, k: WideSmp, input: WideSmp) -> WideSmp {
        let (v1, _v2) = self.integrator.tick(g, k, input);
        k * v1
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: WideSmp) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Notch {
    integrator: Integrator,
}

impl Notch {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, k: WideSmp, input: WideSmp) -> WideSmp {
        let (v1, _v2) = self.integrator.tick(g, k, input);
        k.mul_add(-v1, input)
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: WideSmp) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Peaking {
    integrator: Integrator,
}

impl Peaking {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, k: WideSmp, gain: WideSmp, input: WideSmp) -> WideSmp {
        let gain = gain.max(MIN_GAIN);
        let k = k / gain.sqrt();
        let (v1, _v2) = self.integrator.tick(g, k, input);
        ((gain - ONE) * k).mul_add(v1, input)
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: WideSmp) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy)]
pub(super) struct ShelfCoeffs {
    a: WideSmp,
    sqrt_a: WideSmp,
    gain: WideSmp,
}

impl ShelfCoeffs {
    #[inline(always)]
    pub(super) fn new(gain: WideSmp) -> Self {
        let gain = gain.max(MIN_GAIN);
        let a = gain.sqrt();

        Self {
            a,
            sqrt_a: a.sqrt(),
            gain,
        }
    }

    #[inline(always)]
    pub(super) fn cascaded(gain: WideSmp) -> Self {
        Self::new(gain.max(MIN_GAIN).sqrt())
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct LowShelf {
    integrator: Integrator,
}

impl LowShelf {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, k: WideSmp, coeffs: ShelfCoeffs, input: WideSmp) -> WideSmp {
        let (v1, v2) = self.integrator.tick(g / coeffs.sqrt_a, k, input);
        (coeffs.gain - ONE).mul_add(v2, ((coeffs.a - ONE) * k).mul_add(v1, input))
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: WideSmp) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct HighShelf {
    integrator: Integrator,
}

impl HighShelf {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, k: WideSmp, coeffs: ShelfCoeffs, input: WideSmp) -> WideSmp {
        let (v1, v2) = self.integrator.tick(g * coeffs.sqrt_a, k, input);
        (ONE - coeffs.gain).mul_add(
            v2,
            ((coeffs.a - coeffs.gain) * k).mul_add(v1, coeffs.gain * input),
        )
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: WideSmp) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct OnePoleLowPass {
    integrator: OnePoleIntegrator,
}

impl OnePoleLowPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, input: WideSmp) -> WideSmp {
        self.integrator.tick(g, input)
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: WideSmp) {
        self.integrator.reset(mask);
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct OnePoleHighPass {
    integrator: OnePoleIntegrator,
}

impl OnePoleHighPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: WideSmp, input: WideSmp) -> WideSmp {
        input - self.integrator.tick(g, input)
    }

    #[inline(always)]
    pub(super) fn clear(&mut self, mask: WideSmp) {
        self.integrator.reset(mask);
    }
}
