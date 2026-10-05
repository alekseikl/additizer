#![allow(clippy::new_without_default)]

//! Time-domain and spectral DSP used by Additizer.

pub const NUM_CHANNELS: usize = 2;

pub mod coeffs;
pub mod filters;
pub mod iir_decimator;
pub mod level_ballistics;
pub mod phase;
pub mod smooth;
pub mod stereo_sample;
pub mod types;
pub mod units;

pub use coeffs::{catmull_rom, catmull_rom_from_powers};
pub use iir_decimator::IirDecimator;
pub use level_ballistics::{LevelBallistics, StereoLevelBallistics};
pub use phase::{Phase, PhaseX4};
pub use smooth::{SmoothedSample, SmoothedSampleParams, Smoother};
pub use stereo_sample::StereoSample;
pub use types::{ComplexSample, Sample};
pub use units::{
    C4_NOTE, C4_PITCH, MAX_CUTOFF, MIN_CUTOFF, db_to_gain, db_to_gain_fast, fast_exp2_x4,
    fast_pitch_to_freq_x4, freq_to_c4_pitch, freq_to_pitch, from_ms, from_st, gain_to_db,
    gain_to_db_fast, map_x4, note_to_pitch, pan_gain, pitch_to_freq, power_scale, zip_map_x4,
};
