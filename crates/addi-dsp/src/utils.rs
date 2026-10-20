use crate::Smp;

/// Clamp `value` into `[min, max]`. Non-finite values become `0`.
#[inline]
pub fn sanitize(value: Smp, min: Smp, max: Smp) -> Smp {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        0.0
    }
}
