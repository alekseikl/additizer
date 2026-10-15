use crate::synth_engine::SIMD_LANES;

pub const NUM_CHANNELS: usize = 2;

pub const LEFT_CHANNEL: usize = 0;
pub const RIGHT_CHANNEL: usize = 1;

const KILL_HEADROOM: usize = 4;

pub const AVAILABLE_VOICES: usize = 16;
pub const MAX_VOICES: usize = AVAILABLE_VOICES + KILL_HEADROOM;
pub const VOICE_BLOCK_LEN: usize = SIMD_LANES / NUM_CHANNELS;
pub const VOICE_BLOCKS: usize = MAX_VOICES / VOICE_BLOCK_LEN;

const _: () = assert!(VOICE_BLOCK_LEN * NUM_CHANNELS == SIMD_LANES);
const _: () = assert!(MAX_VOICES.is_multiple_of(VOICE_BLOCK_LEN));
