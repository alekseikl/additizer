use std::hint::black_box;

use additizer::synth_engine::{
    ComplexSample, SPECTRAL_BUFFER_SIZE, Sample,
    filters::{
        control::MAX_PRE_Q,
        spectral_filter::{FilterParams, FilterType, SpectralFilter},
    },
    spectral_filter::MIN_Q_ROLLOFF,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// Octaves of the note fundamental. Two octaves up is the 4th harmonic.
const CUTOFF_OCTAVES: Sample = 2.0;
const Q: Sample = 1.0;
const Q_CUTOFF_OCTAVES: Sample = 8.0;

const FILTER_TYPES: [FilterType; 16] = [
    FilterType::LowPass12,
    FilterType::LowPass18,
    FilterType::LowPass24,
    FilterType::HighPass12,
    FilterType::HighPass18,
    FilterType::HighPass24,
    FilterType::BandPass6,
    FilterType::BandPass12,
    FilterType::BandPass18,
    FilterType::BandPass24,
    FilterType::Peaking,
    FilterType::Notch,
    FilterType::LowShelf12,
    FilterType::LowShelf24,
    FilterType::HighShelf12,
    FilterType::HighShelf24,
];

fn params(linear_phase: bool) -> FilterParams {
    FilterParams {
        drive: 0.0,
        cutoff: CUTOFF_OCTAVES,
        q: Q,
        pre_q: MAX_PRE_Q,
        q_cutoff: Q_CUTOFF_OCTAVES,
        q_rolloff: MIN_Q_ROLLOFF,
        linear_phase,
    }
}

fn spectrum() -> [ComplexSample; SPECTRAL_BUFFER_SIZE] {
    std::array::from_fn(|i| ComplexSample::new(0.5, i as Sample * 1e-4))
}

fn bench_spectral_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectral_filter");
    group.throughput(Throughput::Elements(SPECTRAL_BUFFER_SIZE as u64));

    let input = spectrum();

    for linear_phase in [false, true] {
        let phase = if linear_phase {
            "linear_phase"
        } else {
            "minimum_phase"
        };

        for filter_type in FILTER_TYPES {
            let filter = SpectralFilter::new(filter_type, params(linear_phase));
            let mut output = input;

            group.bench_with_input(
                BenchmarkId::new(phase, format!("{filter_type:?}")),
                &filter_type,
                |b, _| {
                    b.iter(|| {
                        filter.apply_response(black_box(&input[..]), black_box(&mut output[..]));
                        output[SPECTRAL_BUFFER_SIZE - 1].re
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_spectral_filter);
criterion_main!(benches);
