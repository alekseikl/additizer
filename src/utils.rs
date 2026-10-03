pub use additizer_dsp::{
    C4_PITCH, MAX_CUTOFF, MIN_CUTOFF, db_to_gain, db_to_gain_fast, freq_to_c4_pitch, from_ms,
    from_st, gain_to_db, gain_to_db_fast, pitch_to_freq, power_scale,
};
pub use additizer_engine::{MAX_LEVEL_DB, MIN_LEVEL_DB};

macro_rules! log {
    ($($args:tt)*) => {
        ::nice_plug::nice_log!($($args)*)
    };
}
pub(crate) use log;
