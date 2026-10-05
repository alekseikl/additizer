use std::array;

use wide::f32x4;

use crate::synth_engine::{
    coeffs::catmull_rom_from_powers,
    phase::{Phase, PhaseX4},
    types::Sample,
};

use super::{MAX_UNISON_VOICES, UnisonVoice, WaveformBuffer};

pub(super) const UNISON_LANES: usize = 4;
pub(super) const UNISON_CHUNKS: usize = MAX_UNISON_VOICES / UNISON_LANES;
const _: () = assert!(UNISON_CHUNKS * UNISON_LANES == MAX_UNISON_VOICES);

#[derive(Clone, Copy, Default)]
pub(super) struct UnisonLaneParams {
    rate_from: f32x4,
    rate_delta: f32x4,
    phase_shift_from: f32x4,
    phase_shift_delta: f32x4,
    gain_from: f32x4,
    gain_delta: f32x4,
}

impl UnisonLaneParams {
    #[inline(always)]
    pub(super) fn from_voices(voices: &[UnisonVoice]) -> Self {
        debug_assert!(voices.len() <= UNISON_LANES);

        let lanes = |f: fn(&UnisonVoice) -> Sample| {
            f32x4::new(array::from_fn(|i| voices.get(i).map(f).unwrap_or(0.0)))
        };

        let phase_shift_from = PhaseX4::wrap_normalized(lanes(|uv| uv.phase_shift.from));
        let phase_shift_to = PhaseX4::wrap_normalized(lanes(|uv| uv.phase_shift.to));

        Self {
            rate_from: lanes(|uv| uv.rate.from),
            rate_delta: lanes(|uv| uv.rate.to - uv.rate.from),
            phase_shift_from,
            phase_shift_delta: phase_shift_to - phase_shift_from,
            gain_from: lanes(|uv| uv.gain.from),
            gain_delta: lanes(|uv| uv.gain.to - uv.gain.from),
        }
    }
}

struct WaveTap {
    idx: [u32; UNISON_LANES],
    gt: f32x4,
    gt2: f32x4,
    gt3: f32x4,
}

#[derive(Clone, Copy)]
pub(super) struct SampleCtx<'a> {
    pub(super) buff_t: f32x4,
    pub(super) phase_shift: PhaseX4,
    pub(super) phase_inc: f32x4,
    pub(super) wave_from: &'a WaveformBuffer,
    pub(super) wave_to: &'a WaveformBuffer,
    pub(super) acc_from: [f32x4; UNISON_LANES],
    pub(super) acc_to: [f32x4; UNISON_LANES],
}

#[inline(always)]
pub(super) fn render_same<const BITS: usize>(
    phases: &mut [Phase; UNISON_LANES],
    params: &UnisonLaneParams,
    s: &mut SampleCtx,
    lanes: usize,
) {
    let phase = PhaseX4::load(phases);

    let unison_shift = params
        .phase_shift_delta
        .mul_add(s.buff_t, params.phase_shift_from);
    let read_phase = phase + s.phase_shift + PhaseX4::from_wrapped(unison_shift);
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
pub(super) fn render_sized<const FROM_BITS: usize, const TO_BITS: usize>(
    phases: &mut [Phase; UNISON_LANES],
    params: &UnisonLaneParams,
    s: &mut SampleCtx,
    lanes: usize,
) {
    let phase = PhaseX4::load(phases);

    let unison_shift = params
        .phase_shift_delta
        .mul_add(s.buff_t, params.phase_shift_from);
    let read_phase = phase + s.phase_shift + PhaseX4::from_wrapped(unison_shift);
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

#[inline(always)]
fn load_segment(wave_buffer: &WaveformBuffer, idx: usize) -> f32x4 {
    let s = &wave_buffer[idx..idx + 4];

    f32x4::new([s[0], s[1], s[2], s[3]])
}

#[inline(always)]
fn wave_tap<const BITS: usize>(read_phase: PhaseX4, g: f32x4) -> WaveTap {
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
