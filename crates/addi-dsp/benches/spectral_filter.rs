use std::hint::black_box;

use addi_dsp::{
    ComplexSample, Sample,
    filters::{
        control::MAX_PRE_Q,
        spectral_filter::{FilterParams, FilterType, SpectralFilter},
    },
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// Matches the engine spectrum length (`1 << SPECTRUM_BITS`).
const SPECTRAL_BUFFER_SIZE: usize = 1024;
/// Same lower bound as the spectral filter module's Q-rolloff parameter.
const MIN_Q_ROLLOFF: Sample = 3.0;

/// Octaves of the note fundamental. Two octaves up is the 4th harmonic.
const CUTOFF_OCTAVES: Sample = 2.0;
const Q: Sample = 1.0;
const Q_CUTOFF_OCTAVES: Sample = 8.0;

fn params() -> FilterParams {
    FilterParams {
        drive: 0.0,
        cutoff: CUTOFF_OCTAVES,
        q: Q,
        pre_q: MAX_PRE_Q,
        q_cutoff: Q_CUTOFF_OCTAVES,
        q_rolloff: MIN_Q_ROLLOFF,
        linear_phase: true,
    }
}

fn spectrum() -> [ComplexSample; SPECTRAL_BUFFER_SIZE] {
    std::array::from_fn(|i| ComplexSample::new(0.5, i as Sample * 1e-4))
}

fn bench_spectral_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectral_filter");
    group.throughput(Throughput::Elements(SPECTRAL_BUFFER_SIZE as u64));

    let input = spectrum();

    for filter_type in FilterType::ALL {
        let filter = SpectralFilter::new(filter_type, params());
        let mut output = input;

        group.bench_with_input(
            BenchmarkId::new("linear_phase", format!("{filter_type:?}")),
            &filter_type,
            |b, _| {
                b.iter(|| {
                    filter.apply_response(black_box(&input[..]), black_box(&mut output[..]));
                    // Bins are independent, so keep the written buffer observable.
                    black_box(output[SPECTRAL_BUFFER_SIZE - 1])
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_spectral_filter);
criterion_main!(benches);
