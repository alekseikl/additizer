use std::marker::PhantomData;

use super::{BandPass, HighPass, LowPass};
use crate::synth_engine::Sample;

/// Output tap of an SVF stage. Monomorphizing on this (`SvfStage<LowPass>`)
/// selects the tap at compile time.
pub(super) trait Output: Copy {
    fn output(v0: Sample, v1: Sample, v2: Sample, k: Sample) -> Sample;
    fn one_pole(x: Sample, lp: Sample) -> Sample;
}

impl Output for LowPass {
    #[inline(always)]
    fn output(_v0: Sample, _v1: Sample, v2: Sample, _k: Sample) -> Sample {
        v2
    }

    #[inline(always)]
    fn one_pole(_x: Sample, lp: Sample) -> Sample {
        lp
    }
}

impl Output for HighPass {
    #[inline(always)]
    fn output(v0: Sample, v1: Sample, v2: Sample, k: Sample) -> Sample {
        v0 - k * v1 - v2
    }

    #[inline(always)]
    fn one_pole(x: Sample, lp: Sample) -> Sample {
        x - lp
    }
}

impl Output for BandPass {
    #[inline(always)]
    fn output(_v0: Sample, v1: Sample, _v2: Sample, k: Sample) -> Sample {
        k * v1
    }

    #[inline(always)]
    fn one_pole(_x: Sample, lp: Sample) -> Sample {
        lp
    }
}

/// One SVF section. `S` is the output tap, as in `SvfStage<super::LowPass>`.
#[derive(Default, Clone, Copy)]
pub(super) struct SvfStage<S> {
    ic1eq: Sample,
    ic2eq: Sample,
    _output: PhantomData<S>,
}

impl<S: Output> SvfStage<S> {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: Sample, k: Sample, v0: Sample) -> Sample {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * (self.ic1eq + g * v3);
        let v2 = self.ic2eq + g * v1;

        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        S::output(v0, v1, v2, k)
    }
}

/// One-pole section cascaded after an [`SvfStage`].
#[derive(Default, Clone, Copy)]
pub(super) struct OnePoleStage<S> {
    s: Sample,
    _output: PhantomData<S>,
}

impl<S: Output> OnePoleStage<S> {
    #[inline(always)]
    pub(super) fn tick(&mut self, g: Sample, x: Sample) -> Sample {
        let g = g / (1.0 + g);
        let v = (x - self.s) * g;
        let lp = v + self.s;

        self.s = lp + v;

        S::one_pole(x, lp)
    }
}
