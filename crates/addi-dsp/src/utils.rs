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

/// `self` when it is finite, otherwise `0`.
pub(crate) trait SmpUtils {
    fn finite_or_zero(self) -> Self;
}

impl SmpUtils for Smp {
    #[inline]
    fn finite_or_zero(self) -> Self {
        if self.is_finite() { self } else { 0.0 }
    }
}
