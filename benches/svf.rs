use std::{f32::consts::TAU, hint::black_box};

use additizer::synth_engine::{MAX_BLOCK_SIZE, Sample};
use additizer_dsp::filters::{
    control::MAX_PRE_Q,
    svf::{SvfChannel, SvfType},
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

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
    group.throughput(Throughput::Elements(MAX_BLOCK_SIZE as u64));

    let input = sine_block();
    let cutoff = [CUTOFF_OCTAVES; MAX_BLOCK_SIZE];
    let k = [Q.recip(); MAX_BLOCK_SIZE];
    let pre_k = [MAX_PRE_Q.recip(); MAX_BLOCK_SIZE];
    let gain = [1.0; MAX_BLOCK_SIZE];

    for filter_type in FILTER_TYPES {
        let mut state = SvfChannel::new(filter_type);
        let mut output = [0.0; MAX_BLOCK_SIZE];

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{filter_type:?}")),
            &filter_type,
            |b, _| {
                b.iter(|| {
                    state.process(
                        black_box(SAMPLE_RATE),
                        black_box(&input[..]),
                        black_box(&cutoff[..]),
                        black_box(&k[..]),
                        black_box(&pre_k[..]),
                        black_box(&gain[..]),
                        black_box(&mut output[..]),
                    );
                    // Last sample depends on every earlier one, so the loop stays live.
                    black_box(output[MAX_BLOCK_SIZE - 1])
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_svf);
criterion_main!(benches);
