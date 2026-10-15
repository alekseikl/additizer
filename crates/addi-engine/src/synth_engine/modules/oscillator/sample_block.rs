use std::array;

use wide::f32x4;

use crate::synth_engine::{
    SIMD_LANES,
    fast_math::{CATMULL_TAPS, catmull_rom_from_powers},
    phase::{Phase, WidePhase},
    types::{Smp, WideSmp},
};

use super::{MAX_UNISON_VOICES, UnisonVoice, WaveformBuffer};

pub const UNISON_CHUNKS: usize = MAX_UNISON_VOICES / SIMD_LANES;
const _: () = assert!(UNISON_CHUNKS * SIMD_LANES == MAX_UNISON_VOICES);

#[derive(Clone, Copy, Default)]
pub struct UnisonBlockParams {
    rate_from: WideSmp,
    rate_delta: WideSmp,
    phase_shift_from: WideSmp,
    phase_shift_delta: WideSmp,
    gain_from: WideSmp,
    gain_delta: WideSmp,
}

impl UnisonBlockParams {
    /// Voice gains are already normalized by the unison layout.
    #[inline(always)]
    pub fn from_voices(voices: &[UnisonVoice]) -> Self {
        debug_assert!(voices.len() <= SIMD_LANES);

        let lanes = |f: fn(&UnisonVoice) -> Smp| {
            WideSmp::new(array::from_fn(|i| voices.get(i).map(f).unwrap_or(0.0)))
        };

        let phase_shift_from = lanes(|uv| uv.phase_shift.from);
        let phase_shift_to = lanes(|uv| uv.phase_shift.to);
        let gain_from = lanes(|uv| uv.gain.from);
        let gain_to = lanes(|uv| uv.gain.to);

        Self {
            rate_from: lanes(|uv| uv.rate.from),
            rate_delta: lanes(|uv| uv.rate.to - uv.rate.from),
            phase_shift_from,
            phase_shift_delta: phase_shift_to - phase_shift_from,
            gain_from,
            gain_delta: gain_to - gain_from,
        }
    }
}

struct WaveTap {
    idx: [u32; SIMD_LANES],
    gt: WideSmp,
    gt2: WideSmp,
    gt3: WideSmp,
}

#[derive(Clone, Copy)]
pub struct SampleCtx<'a> {
    pub buff_t: WideSmp,
    pub phase_shift: WidePhase,
    pub phase_inc: WideSmp,
    pub wave_from: &'a WaveformBuffer,
    pub wave_to: &'a WaveformBuffer,
    pub acc_from: [f32x4; SIMD_LANES],
    pub acc_to: [f32x4; SIMD_LANES],
}

#[inline(always)]
pub fn render_same<const BITS: usize>(
    phases: &mut [Phase; SIMD_LANES],
    params: &UnisonBlockParams,
    s: &mut SampleCtx,
    lanes: usize,
) {
    let phase = WidePhase::load(phases);

    let unison_shift = params
        .phase_shift_delta
        .mul_add(s.buff_t, params.phase_shift_from);
    let read_phase = phase + s.phase_shift + WidePhase::from_normalized(unison_shift);
    let idx = read_phase.wave_index::<BITS>();
    let t = read_phase.wave_index_fraction::<BITS>();
    let g = params.gain_delta.mul_add(s.buff_t, params.gain_from);
    let gt = g * t;
    let gt2 = gt * t;
    let gt3 = gt2 * t;

    let rate = params.rate_delta.mul_add(s.buff_t, params.rate_from);
    (phase + rate * s.phase_inc).store(phases);

    for (lane, wave_idx) in idx.into_iter().enumerate().take(lanes) {
        let weights = catmull_rom_from_powers(
            g.as_array()[lane],
            gt.as_array()[lane],
            gt2.as_array()[lane],
            gt3.as_array()[lane],
        );

        let from = load_segment(s.wave_from, wave_idx as usize);
        let to = load_segment(s.wave_to, wave_idx as usize);
        s.acc_from[lane] = from.mul_add(weights, s.acc_from[lane]);
        s.acc_to[lane] = to.mul_add(weights, s.acc_to[lane]);
    }
}

#[inline(always)]
pub fn render_sized<const FROM_BITS: usize, const TO_BITS: usize>(
    phases: &mut [Phase; SIMD_LANES],
    params: &UnisonBlockParams,
    s: &mut SampleCtx,
    lanes: usize,
) {
    let phase = WidePhase::load(phases);

    let unison_shift = params
        .phase_shift_delta
        .mul_add(s.buff_t, params.phase_shift_from);
    let read_phase = phase + s.phase_shift + WidePhase::from_normalized(unison_shift);
    let g = params.gain_delta.mul_add(s.buff_t, params.gain_from);
    let from_tap = wave_tap::<FROM_BITS>(read_phase, g);
    let to_tap = wave_tap::<TO_BITS>(read_phase, g);

    let rate = params.rate_delta.mul_add(s.buff_t, params.rate_from);
    (phase + rate * s.phase_inc).store(phases);

    for lane in 0..lanes {
        let g = g.as_array()[lane];
        let from_weights = catmull_rom_from_powers(
            g,
            from_tap.gt.as_array()[lane],
            from_tap.gt2.as_array()[lane],
            from_tap.gt3.as_array()[lane],
        );
        let to_weights = catmull_rom_from_powers(
            g,
            to_tap.gt.as_array()[lane],
            to_tap.gt2.as_array()[lane],
            to_tap.gt3.as_array()[lane],
        );

        let from = load_segment(s.wave_from, from_tap.idx[lane] as usize);
        let to = load_segment(s.wave_to, to_tap.idx[lane] as usize);
        s.acc_from[lane] = from.mul_add(from_weights, s.acc_from[lane]);
        s.acc_to[lane] = to.mul_add(to_weights, s.acc_to[lane]);
    }
}

/// Sum the per-voice tap vectors in this chunk. Unused voices stay zero.
///
/// Adjacent pairs, so the reduction tree stays balanced.
#[inline(always)]
pub fn sum_lanes(acc: &[f32x4; SIMD_LANES]) -> f32x4 {
    let mut level = *acc;
    let mut n = SIMD_LANES;

    while n > 1 {
        let mut i = 0;

        while i < n {
            level[i / 2] = level[i] + level[i + 1];
            i += 2;
        }

        n /= 2;
    }

    level[0]
}

#[inline(always)]
fn load_segment(wave_buffer: &WaveformBuffer, idx: usize) -> f32x4 {
    let s = &wave_buffer[idx..idx + CATMULL_TAPS];

    f32x4::new([s[0], s[1], s[2], s[3]])
}

#[inline(always)]
fn wave_tap<const BITS: usize>(read_phase: WidePhase, g: WideSmp) -> WaveTap {
    let t = read_phase.wave_index_fraction::<BITS>();
    let gt = g * t;
    let gt2 = gt * t;
    let gt3 = gt2 * t;

    WaveTap {
        idx: read_phase.wave_index::<BITS>(),
        gt,
        gt2,
        gt3,
    }
}
