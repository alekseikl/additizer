use super::Smp;

/// Silence floor for dB level parameters. At or below this, gain is treated as zero.
pub const MIN_LEVEL_DB: Smp = -60.0;
/// Upper clamp for dB level parameters.
pub const MAX_LEVEL_DB: Smp = 24.0;
