use crate::synth_engine::Sample;

pub use additizer_dsp::{
    C4_PITCH, MAX_CUTOFF, MIN_CUTOFF, db_to_gain, db_to_gain_fast, freq_to_c4_pitch, from_ms,
    from_st, gain_to_db, gain_to_db_fast, note_to_pitch, pan_gain, pitch_to_freq, power_scale,
};

/// Silence floor for dB level parameters. At or below this, gain is treated as zero.
pub const MIN_LEVEL_DB: Sample = -60.0;
/// Upper clamp for dB level parameters.
pub const MAX_LEVEL_DB: Sample = 24.0;

macro_rules! log {
    ($($args:tt)*) => {
        ::nice_plug::nice_log!($($args)*)
    };
}
pub(crate) use log;

pub struct NthElement {
    mul: isize,
    add: isize,
    inverted: bool,
}

impl NthElement {
    pub fn new(mul: isize, add: isize, inverted: bool) -> Self {
        Self { mul, add, inverted }
    }

    pub fn matches(&self, harmonic: usize) -> bool {
        let harmonic = harmonic as isize;
        let result = if self.mul == 0 {
            harmonic == self.add
        } else {
            let diff = harmonic - self.add;
            diff % self.mul == 0 && diff / self.mul >= 0
        };

        result ^ self.inverted
    }
}
