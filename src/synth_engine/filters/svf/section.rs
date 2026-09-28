use crate::synth_engine::Sample;

/// Trapezoidal integrator pair shared by every two-pole section.
#[derive(Default, Clone, Copy)]
struct Integrator {
    ic1eq: Sample,
    ic2eq: Sample,
}

impl Integrator {
    /// Returns `(v1, v2)` for input `v0`.
    #[inline(always)]
    fn tick(&mut self, g: Sample, k: Sample, v0: Sample) -> (Sample, Sample) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * (self.ic1eq + g * v3);
        let v2 = self.ic2eq + g * v1;

        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        (v1, v2)
    }
}

/// One-pole integrator. `tick` returns the lowpass.
#[derive(Default, Clone, Copy)]
struct OnePoleIntegrator {
    s: Sample,
}

impl OnePoleIntegrator {
    #[inline(always)]
    fn tick(&mut self, g: Sample, input: Sample) -> Sample {
        let g = g / (1.0 + g);
        let v = (input - self.s) * g;
        let lp = v + self.s;

        self.s = lp + v;

        lp
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct LowPass {
    integrator: Integrator,
}

impl LowPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let (_v1, v2) = self.integrator.tick(g, k, input);

        v2
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct HighPass {
    integrator: Integrator,
}

impl HighPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let (v1, v2) = self.integrator.tick(g, k, input);

        input - k * v1 - v2
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct BandPass {
    integrator: Integrator,
}

impl BandPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let (v1, _v2) = self.integrator.tick(g, k, input);

        k * v1
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct Notch {
    integrator: Integrator,
}

impl Notch {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: Sample, k: Sample, input: Sample) -> Sample {
        let (v1, _v2) = self.integrator.tick(g, k, input);

        input - k * v1
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct Peaking {
    integrator: Integrator,
}

impl Peaking {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: Sample, k: Sample, gain: Sample, input: Sample) -> Sample {
        let gain = gain.max(1e-4);
        let k = k / gain.sqrt();
        let (v1, _v2) = self.integrator.tick(g, k, input);
        // Passband stays at unity; `gain` is the level at the cutoff.
        input + (gain - 1.0) * k * v1
    }
}

/// `A = √gain` and `√A`, shared by every section of one shelf.
#[derive(Clone, Copy)]
pub(super) struct ShelfCoeffs {
    /// `A = √gain`.
    a: Sample,
    /// `√A`. Low shelf tunes at `g / sqrt_a`, high shelf at `g * sqrt_a`.
    sqrt_a: Sample,
    gain: Sample,
}

impl ShelfCoeffs {
    #[inline(always)]
    pub(super) fn new(gain: Sample) -> Self {
        let gain = gain.max(1e-4);
        let a = gain.sqrt();

        Self {
            a,
            sqrt_a: a.sqrt(),
            gain,
        }
    }

    /// Both sections of a 24 dB shelf run at `√gain`.
    #[inline(always)]
    pub(super) fn cascaded(gain: Sample) -> Self {
        Self::new(gain.max(1e-4).sqrt())
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct LowShelf {
    integrator: Integrator,
}

impl LowShelf {
    #[inline(always)]
    pub(super) fn tick(
        &mut self,
        g: Sample,
        k: Sample,
        coeffs: ShelfCoeffs,
        input: Sample,
    ) -> Sample {
        // Poles sit at fc / √A so the shelf centers on the cutoff.
        let (v1, v2) = self.integrator.tick(g / coeffs.sqrt_a, k, input);

        input + (coeffs.a - 1.0) * k * v1 + (coeffs.gain - 1.0) * v2
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct HighShelf {
    integrator: Integrator,
}

impl HighShelf {
    #[inline(always)]
    pub(super) fn tick(
        &mut self,
        g: Sample,
        k: Sample,
        coeffs: ShelfCoeffs,
        input: Sample,
    ) -> Sample {
        // Poles sit at fc * √A so the shelf centers on the cutoff.
        let (v1, v2) = self.integrator.tick(g * coeffs.sqrt_a, k, input);

        coeffs.gain * input + (coeffs.a - coeffs.gain) * k * v1 + (1.0 - coeffs.gain) * v2
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct OnePoleLowPass {
    integrator: OnePoleIntegrator,
}

impl OnePoleLowPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: Sample, input: Sample) -> Sample {
        self.integrator.tick(g, input)
    }
}

#[derive(Default, Clone, Copy)]
pub(super) struct OnePoleHighPass {
    integrator: OnePoleIntegrator,
}

impl OnePoleHighPass {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: Sample, input: Sample) -> Sample {
        input - self.integrator.tick(g, input)
    }
}
