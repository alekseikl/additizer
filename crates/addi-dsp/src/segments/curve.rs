use crate::{
    SIMD_LANES, Smp, WideSmp,
    fast_math::{fast_exp2_wide, fast_sin_pi_wide},
    simd_utils::fill,
};

/// Walk one segment from `position` 0 at the start to 1 at the end.
///
/// `step` is how far `width` advances each sample. `Ok(position)` means `out` was filled
/// and the segment is still running. `Err(written)` means it ended after `written` samples,
/// or `0` when it had none left. A non-positive `step` holds `position`.
pub(super) fn render_segment(
    out: &mut [Smp],
    position: Smp,
    step: Smp,
    width: Smp,
    origin: Smp,
    end: Smp,
    power: Smp,
    smooth: bool,
) -> Result<Smp, usize> {
    if width <= 0.0 || position >= 1.0 {
        return Err(0);
    }

    if step <= 0.0 {
        Curve::render(out, position, 0.0, origin, end, power, smooth);
        return Ok(position);
    }

    let t_step = step * width.recip();
    let remaining = (1.0 - position).max(0.0) * width;
    let n = ((remaining / step) as usize).min(out.len());

    if n == 0 {
        return Err(0);
    }

    Curve::render(&mut out[..n], position, t_step, origin, end, power, smooth);

    let position = (position + n as Smp * step / width).min(1.0);

    if n < out.len() || position >= 1.0 {
        Err(n)
    } else {
        Ok(position)
    }
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
    /// `t` is 0 at `origin` and 1 at `end`.
    pub(super) fn render(
        out: &mut [Smp],
        t: Smp,
        t_step: Smp,
        origin: Smp,
        end: Smp,
        power: Smp,
        smooth: bool,
    ) {
        let t_step = WideSmp::splat(t_step);
        let mut curve = Curve {
            out,
            t: WideSmp::splat(t) + WideSmp::new([0.0, 1.0, 2.0, 3.0]) * t_step,
            step: t_step * WideSmp::splat(SIMD_LANES as Smp),
            origin: WideSmp::splat(origin),
            delta: WideSmp::splat(end - origin),
            power: PowerScale::new(power),
        };

        match (smooth, curve.power.curved) {
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
