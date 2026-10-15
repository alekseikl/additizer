#![allow(clippy::new_without_default)]

//! Time-domain and spectral DSP used by Additizer.

pub const NUM_CHANNELS: usize = 2;

pub mod consts;
pub mod fast_math;
pub mod filters;
pub mod iir_decimator;
pub mod level_ballistics;
pub mod math;
pub mod phase;
pub mod simd_utils;
pub mod smooth;
pub mod stereo_sample;
pub mod types;

pub use consts::{C4_NOTE, C4_PITCH, MAX_CUTOFF, MIN_CUTOFF};
pub use fast_math::{
    catmull_rom, catmull_rom_from_powers, db_to_gain_fast_wide, fast_exp2_wide,
    fast_pitch_to_freq_wide,
};
pub use iir_decimator::IirDecimator;
pub use level_ballistics::{LevelBallistics, StereoLevelBallistics};
pub use math::{
    db_to_gain, db_to_gain_fast, freq_to_c4_pitch, freq_to_pitch, from_ms, from_st, gain_to_db,
    gain_to_db_fast, note_to_pitch, pan_gain, pitch_to_freq, power_scale,
};
pub use phase::{Phase, WidePhase};
pub use simd_utils::{fill_wide, map_wide, map_wide_in_place, zip_map_wide};
pub use smooth::{SmoothedSample, SmoothedSampleParams, Smoother};
pub use stereo_sample::StereoSample;
pub use types::{ComplexSample, SIMD_LANES, Smp, WideSmp};
