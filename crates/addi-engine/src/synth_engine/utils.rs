use super::Sample;

/// Silence floor for dB level parameters. At or below this, gain is treated as zero.
pub const MIN_LEVEL_DB: Sample = -60.0;
/// Upper clamp for dB level parameters.
pub const MAX_LEVEL_DB: Sample = 24.0;
