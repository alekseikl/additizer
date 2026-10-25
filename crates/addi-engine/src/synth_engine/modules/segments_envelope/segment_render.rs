use addi_dsp::segments::{self, Segment};

use crate::synth_engine::Smp;

/// `Ok(position)` — this segment filled `out` and is still running.
/// `Err(written)` — this segment ended after `written` samples (`0` if it had none left).
///
/// `position` is normalized progress through the segment. `phase_step` is the curve
/// advanced per sample. A non-positive step holds the current level. The step stays
/// constant for the fill, the same way an envelope stage uses one sample interval.
pub(super) fn render_segment(
    out: &mut [Smp],
    position: Smp,
    phase_step: Smp,
    start: Segment,
    end: Segment,
) -> Result<Smp, usize> {
    let width = end.x - start.x;

    if width <= 0.0 || position >= 1.0 {
        return Err(0);
    }

    let phase = start.x + position * width;

    if phase_step <= 0.0 {
        segments::render_segment(out, phase, 0.0, start, end);
        return Ok(position);
    }

    let remaining = (1.0 - position).max(0.0) * width;
    let n = ((remaining / phase_step) as usize).min(out.len());

    if n == 0 {
        return Err(0);
    }

    segments::render_segment(&mut out[..n], phase, phase_step, start, end);

    let position = (position + n as Smp * phase_step / width).min(1.0);

    if n < out.len() || position >= 1.0 {
        Err(n)
    } else {
        Ok(position)
    }
}
