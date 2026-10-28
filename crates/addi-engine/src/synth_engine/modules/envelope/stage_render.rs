use std::array;

use addi_dsp::fast_exp2_wide;

use crate::synth_engine::{
    fill,
    types::{SIMD_LANES, Smp, WideSmp},
};

/// `Ok(position)` — this stage filled `out` and is still running.
/// `Err(written)` — this stage ended after `written` samples (`0` if it had none left).
fn render_timed(
    out: &mut [Smp],
    position: Smp,
    duration: Smp,
    t_step: Smp,
    write: impl FnOnce(&mut [Smp], Smp),
) -> Result<Smp, usize> {
    if duration <= 0.0 {
        return Err(0);
    }

    let remaining = (1.0 - position).max(0.0) * duration;
    let n = ((remaining / t_step) as usize).min(out.len());

    if n == 0 {
        return Err(0);
    }

    write(&mut out[..n], position);

    if n < out.len() {
        Err(n)
    } else {
        Ok((position + n as Smp * t_step / duration).min(1.0))
    }
}

/// Timed stage held at a constant `level`. `position` is normalized progress.
pub(super) fn render_level(
    out: &mut [Smp],
    position: Smp,
    duration: Smp,
    t_step: Smp,
    level: Smp,
) -> Result<Smp, usize> {
    render_timed(out, position, duration, t_step, |out, _| out.fill(level))
}

/// Timed stage that ramps from `from` to `to`. `position` is normalized progress.
pub(super) fn render_curve(
    out: &mut [Smp],
    position: Smp,
    duration: Smp,
    t_step: Smp,
    from: Smp,
    to: Smp,
    power: Smp,
) -> Result<Smp, usize> {
    render_timed(out, position, duration, t_step, |out, position| {
        fill_curve(out, position, duration, t_step, from, to, power);
    })
}

/// Linear ramp `from + interval * x`, one vector of lanes at a time.
/// `x` starts at normalized `position` and advances by `dx` per sample.
#[inline(never)]
fn fill_linear_ramp(out: &mut [Smp], position: Smp, dx: Smp, interval: Smp, from: Smp) {
    let interval_x4 = WideSmp::splat(interval);
    let from_x4 = WideSmp::splat(from);
    let step = WideSmp::splat(dx * SIMD_LANES as Smp);
    let lanes: [Smp; SIMD_LANES] = array::from_fn(|lane| position + lane as Smp * dx);
    let mut x = WideSmp::new(lanes);

    fill(out, || {
        let y = interval_x4.mul_add(x, from_x4);
        x += step;
        y
    });
}

/// Curved segment, one vector of lanes at a time. Outlined so it stays a leaf.
/// `x` starts at normalized `position` and advances by `dx` per sample.
#[inline(never)]
fn fill_curved(out: &mut [Smp], position: Smp, dx: Smp, interval: Smp, from: Smp, power: Smp) {
    // power_scale(x) = (exp(p * x) - 1) / (exp(p) - 1), so
    // interval * power_scale(x) + from = scale * exp(p * x) + offset.
    let scale = interval / (power.exp() - 1.0);
    let offset = from - scale;

    const LOG2_E: WideSmp = WideSmp::splat(std::f32::consts::LOG2_E);
    let power_log2e = WideSmp::splat(power) * LOG2_E;
    let step = WideSmp::splat(dx * SIMD_LANES as Smp);
    let scale_x4 = WideSmp::splat(scale);
    let offset_x4 = WideSmp::splat(offset);
    let lanes: [Smp; SIMD_LANES] = array::from_fn(|lane| position + lane as Smp * dx);
    let mut x = WideSmp::new(lanes);

    fill(out, || {
        let exp = fast_exp2_wide(x * power_log2e);
        let y = scale_x4.mul_add(exp, offset_x4);
        x += step;
        y
    });
}

fn fill_curve(
    out: &mut [Smp],
    position: Smp,
    duration: Smp,
    t_step: Smp,
    from: Smp,
    to: Smp,
    power: Smp,
) {
    let dx = t_step * duration.recip();
    let interval = to - from;

    // Same cutoff as `power_scale`: a flat slope is a linear ramp.
    if power.abs() < 0.005 {
        fill_linear_ramp(out, position, dx, interval, from);
    } else {
        fill_curved(out, position, dx, interval, from, power);
    }
}
