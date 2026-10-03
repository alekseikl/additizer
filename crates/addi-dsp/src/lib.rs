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

pub use coeffs::catmull_rom;
pub use iir_decimator::IirDecimator;
pub use level_ballistics::{LevelBallistics, StereoLevelBallistics};
pub use phase::Phase;
pub use smooth::{SmoothedSample, SmoothedSampleParams, Smoother};
pub use stereo_sample::StereoSample;
pub use types::{ComplexSample, Sample};
pub use units::{
    C4_NOTE, C4_PITCH, MAX_CUTOFF, MIN_CUTOFF, db_to_gain, db_to_gain_fast, freq_to_c4_pitch,
    freq_to_pitch, from_ms, from_st, gain_to_db, gain_to_db_fast, note_to_pitch, pan_gain,
    pitch_to_freq, power_scale,
};
