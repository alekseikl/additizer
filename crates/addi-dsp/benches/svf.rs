use std::{f32::consts::TAU, hint::black_box};

use addi_dsp::{
    Sample,
    filters::{
        control::MAX_PRE_Q,
        svf::{LaneInputs, SvfState, SvfType},
    },
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// 48 kHz host block.
const MAX_BLOCK_SIZE: usize = 128;

const SAMPLE_RATE: Sample = 48_000.0;
/// Octaves relative to C4. Two octaves up is about 1 kHz.
const CUTOFF_OCTAVES: Sample = 2.0;
/// Resonant-stage Q. `k` passed to the filter is `1/Q`.
const Q: Sample = 1.0;

const FILTER_TYPES: [SvfType; 16] = [
    SvfType::LowPass12,
    SvfType::LowPass18,
    SvfType::LowPass24,
    SvfType::HighPass12,
    SvfType::HighPass18,
    SvfType::HighPass24,
    SvfType::BandPass6,
    SvfType::BandPass12,
    SvfType::BandPass18,
    SvfType::BandPass24,
    SvfType::Peaking,
    SvfType::Notch,
    SvfType::LowShelf12,
    SvfType::LowShelf24,
    SvfType::HighShelf12,
    SvfType::HighShelf24,
];

fn sine_block() -> [Sample; MAX_BLOCK_SIZE] {
    std::array::from_fn(|i| (TAU * 440.0 * i as Sample / SAMPLE_RATE).sin())
}

fn bench_svf(c: &mut Criterion) {
    let mut group = c.benchmark_group("svf");
    group.throughput(Throughput::Elements((MAX_BLOCK_SIZE * 4) as u64));

    let input = sine_block();
    let cutoff = [CUTOFF_OCTAVES; MAX_BLOCK_SIZE];
    let k = [Q.recip(); MAX_BLOCK_SIZE];
    let pre_k = [MAX_PRE_Q.recip(); MAX_BLOCK_SIZE];
    let gain = [1.0; MAX_BLOCK_SIZE];
    let audio = [&input[..], &input[..], &input[..], &input[..]];
    let cutoff = [&cutoff[..], &cutoff[..], &cutoff[..], &cutoff[..]];
    let k = [&k[..], &k[..], &k[..], &k[..]];
    let pre_k = [&pre_k[..], &pre_k[..], &pre_k[..], &pre_k[..]];
    let gain = [&gain[..], &gain[..], &gain[..], &gain[..]];

    for filter_type in FILTER_TYPES {
        let mut state = SvfState::new(filter_type);
        let mut output = [[0.0; MAX_BLOCK_SIZE]; 4];

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{filter_type:?}")),
            &filter_type,
            |b, _| {
                b.iter(|| {
                    let [o0, o1, o2, o3] = output.each_mut();

                    state.process(
                        black_box(SAMPLE_RATE),
                        MAX_BLOCK_SIZE,
                        black_box(LaneInputs {
                            audio,
                            cutoff,
                            k,
                            pre_k,
                            gain,
                        }),
                        [o0, o1, o2, o3],
                    );
                    // Last sample depends on every earlier one, so the loop stays live.
                    black_box(output[0][MAX_BLOCK_SIZE - 1])
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_svf);
criterion_main!(benches);
