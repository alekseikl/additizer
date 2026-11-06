use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::{
    SIMD_LANES, Smp, WideSmp,
    fast_math::{fast_exp2_wide, fast_sin_pi_wide},
    simd_utils::fill,
};

/// Bend of one segment. `power` pulls the curve toward its start or its end.
/// `smooth` eases the progress through a sine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CurveParam {
    pub power: Smp,
    pub smooth: bool,
}

/// Walk one segment from `position` 0 at the start to 1 at the end.
///
/// `step` is added to `position` each sample. `Ok(position)` means `out` was filled and the
/// segment is still running. `Err(written)` means it ended after `written` samples, or `0`
/// when it had none left. A non-positive `step` holds `position`.
pub fn render_segment(
    out: &mut [Smp],
    position: Smp,
    step: Smp,
    level: Range<Smp>,
    curve: CurveParam,
) -> Result<Smp, usize> {
    if step <= 0.0 {
        Curve::render(out, position.clamp(0.0, 1.0), 0.0, level, curve);
        return Ok(position);
    }

    if position >= 1.0 {
        return Err(0);
    }

    let n = (((1.0 - position) / step) as usize).min(out.len());

    if n == 0 {
        return Err(0);
    }

    Curve::render(&mut out[..n], position, step, level, curve);

    let position = position + n as Smp * step;

    if n < out.len() || position >= 1.0 {
        Err(n)
    } else {
        Ok(position)
    }
}

/// Exclusive end index of the span that finishes at `x` on a grid of `steps` intervals.
pub(super) fn sample_end(x: Smp, steps: usize) -> usize {
    if x <= 0.0 {
        return 0;
    }

    ((x * steps as Smp).floor() + 1.0).min(steps as Smp + 1.0) as usize
}

pub(super) struct Curve<'a> {
    out: &'a mut [Smp],
    t: WideSmp,
    step: WideSmp,
    origin: WideSmp,
    delta: WideSmp,
    power: PowerScale,
}

impl Curve<'_> {
    /// Fill `out` along a segment. Sample `i` is read at progress `t + i * t_step`.
    /// `t` is 0 at `level.start` and 1 at `level.end`.
    pub(super) fn render(
        out: &mut [Smp],
        t: Smp,
        t_step: Smp,
        level: Range<Smp>,
        param: CurveParam,
    ) {
        let t_step = WideSmp::splat(t_step);
        let mut curve = Curve {
            out,
            t: WideSmp::splat(t) + WideSmp::new([0.0, 1.0, 2.0, 3.0]) * t_step,
            step: t_step * WideSmp::splat(SIMD_LANES as Smp),
            origin: WideSmp::splat(level.start),
            delta: WideSmp::splat(level.end - level.start),
            power: PowerScale::new(param.power),
        };

        match (param.smooth, curve.power.curved) {
            (false, false) => curve.fill::<false, false>(),
            (false, true) => curve.fill::<false, true>(),
            (true, false) => curve.fill::<true, false>(),
            (true, true) => curve.fill::<true, true>(),
        }
    }

    #[inline(always)]
    fn fill<const SMOOTH: bool, const CURVED: bool>(&mut self) {
        fill(self.out, || {
            let progress = if SMOOTH {
                fast_sin_pi_wide(self.t - WideSmp::HALF).mul_add(WideSmp::HALF, WideSmp::HALF)
            } else {
                self.t
            };

            self.t += self.step;

            self.power
                .scale::<CURVED>(progress)
                .mul_add(self.delta, self.origin)
        });
    }
}

struct PowerScale {
    curved: bool,
    power_log2e: WideSmp,
    scale: WideSmp,
}

impl PowerScale {
    fn new(power: Smp) -> Self {
        if power.abs() < 0.005 {
            return Self {
                curved: false,
                power_log2e: WideSmp::ZERO,
                scale: WideSmp::ONE,
            };
        }

        let power_log2e = WideSmp::splat(power * std::f32::consts::LOG2_E);
        let denom = (fast_exp2_wide(power_log2e) - WideSmp::ONE).to_array()[0];
        let scale = WideSmp::splat(denom.recip());

        Self {
            curved: true,
            power_log2e,
            scale,
        }
    }

    #[inline(always)]
    fn scale<const CURVED: bool>(&self, t: WideSmp) -> WideSmp {
        if !CURVED {
            return t;
        }

        let exp = fast_exp2_wide(t * self.power_log2e);

        (exp - WideSmp::ONE) * self.scale
    }
}
