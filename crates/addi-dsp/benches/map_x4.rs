//! Temporary comparison of the hand-written four-lane pitch loop and `map_x4`.
//! Delete this bench once the check is done.

use std::hint::black_box;

use addi_dsp::{Sample, fast_pitch_to_freq_x4, map_x4, phase::Phase};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use wide::f32x4;

/// Matches `addi_engine::MAX_BLOCK_SIZE`.
const MAX_BLOCK_SIZE: usize = 128;
const SAMPLE_RATE: Sample = 48_000.0;

fn pitch_block() -> [Sample; MAX_BLOCK_SIZE] {
    std::array::from_fn(|i| (i as Sample) * 0.01 - 1.0)
}

/// Loop that `pitch_to_phase_inc` used before `map_x4` owned the chunking.
fn before_map_x4(pitch: &[Sample], out: &mut [Sample], freq_phase_mult: Sample) {
    let mult = f32x4::splat(freq_phase_mult);
    let (pitch_chunks, pitch_rem) = pitch.as_chunks::<4>();
    let (out_chunks, out_rem) = out.as_chunks_mut::<4>();

    for (out, pitch) in out_chunks.iter_mut().zip(pitch_chunks) {
        *out = (fast_pitch_to_freq_x4(f32x4::new(*pitch)) * mult).to_array();
    }

    let tail = pitch_rem.len().min(out_rem.len());

    if tail > 0 {
        let mut lanes = [0.0; 4];
        lanes[..tail].copy_from_slice(&pitch_rem[..tail]);
        let freqs = (fast_pitch_to_freq_x4(f32x4::new(lanes)) * mult).to_array();
        out_rem[..tail].copy_from_slice(&freqs[..tail]);
    }
}

fn after_map_x4(pitch: &[Sample], out: &mut [Sample], freq_phase_mult: Sample) {
    let mult = f32x4::splat(freq_phase_mult);

    map_x4(pitch, out, |pitch| fast_pitch_to_freq_x4(pitch) * mult);
}

fn bench_map_x4(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_x4");
    let pitch = pitch_block();
    let mult = Phase::freq_phase_mult(SAMPLE_RATE);

    // 128 is a full block (no tail). 127 forces the zero-padded remainder.
    for len in [MAX_BLOCK_SIZE, MAX_BLOCK_SIZE - 1] {
        group.throughput(Throughput::Elements(len as u64));
        let mut manual_out = [0.0; MAX_BLOCK_SIZE];
        let mut mapped_out = [0.0; MAX_BLOCK_SIZE];

        group.bench_with_input(BenchmarkId::new("before", len), &len, |b, &len| {
            b.iter(|| {
                before_map_x4(
                    black_box(&pitch[..len]),
                    black_box(&mut manual_out[..len]),
                    black_box(mult),
                );
                black_box(manual_out[len - 1])
            });
        });

        group.bench_with_input(BenchmarkId::new("after", len), &len, |b, &len| {
            b.iter(|| {
                after_map_x4(
                    black_box(&pitch[..len]),
                    black_box(&mut mapped_out[..len]),
                    black_box(mult),
                );
                black_box(mapped_out[len - 1])
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_map_x4);
criterion_main!(benches);
