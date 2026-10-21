//! `SpectralFilter::process` for every filter type at 16 voices.
//!
//! Cutoff is +2 octaves, resonant-stage Q is 1, Q-cutoff is 8 octaves, and
//! Q-rolloff is the module minimum, matching `addi-dsp`'s `spectral_filter`
//! bench. Linear phase matches that bench too. Voices sit on C4, so key
//! tracking adds no cutoff offset. An auxiliary spectral module writes a
//! full-length sawtooth once; only `SpectralFilter::process` is timed.

use std::f32::consts::{PI, TAU};
use std::hint::black_box;

use addi_dsp::filters::{
    control::{MAX_RESONANCE_Q, ZERO_RESONANCE_Q},
    spectral_filter::FilterType,
};
use addi_engine::{
    ComplexSample, DEFAULT_TEMPO, NUM_CHANNELS, PlayingVoice, SPECTRAL_BUFFER_SIZE,
    SmoothedSampleParams, Smp, SpectralBuffer, SpectralFilter, StereoSample,
    routing::{Input, InputSlots, OutputsArena, ProcessContext, ProcessParams},
    spectral_filter::{MIN_Q_ROLLOFF, SpectralFilterConfig},
    stub::AudioEnd,
    synth_module::SynthModule,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[path = "aux/mod.rs"]
mod aux;

use aux::Spectral;

const SAMPLE_RATE: Smp = 48_000.0;
const BUFFER_SIZE: usize = 2;

const VOICES: usize = 16;

/// Octaves of the note fundamental. Two octaves up is the 4th harmonic.
const CUTOFF_OCTAVES: Smp = 2.0;

/// Resonant-stage Q, matching `crates/addi-dsp/benches/spectral_filter.rs`.
const Q: Smp = 1.0;

/// Octaves. At and above this, Q is not reduced.
const Q_CUTOFF_OCTAVES: Smp = 8.0;

fn resonance_for_q(q: Smp) -> Smp {
    ((q - ZERO_RESONANCE_Q) / (MAX_RESONANCE_Q - ZERO_RESONANCE_Q)).cbrt()
}

/// Amplitude `1 / (n π)`, alternating quadrature phase. Same bins the harmonic
/// editor writes for its default sawtooth, across the whole spectrum.
fn sawtooth() -> SpectralBuffer {
    let mut spectrum = [ComplexSample::ZERO; SPECTRAL_BUFFER_SIZE];

    for (harmonic, bin) in spectrum.iter_mut().enumerate().skip(1) {
        let phase = if harmonic & 1 == 0 { 0.25 } else { 0.75 };

        *bin = ComplexSample::from_polar(1.0 / (harmonic as Smp * PI), phase * TAU);
    }

    spectrum
}

fn voices() -> [PlayingVoice; VOICES] {
    std::array::from_fn(|i| PlayingVoice::new(i as u8, 60))
}

fn process_params<'a>(voices: &'a [PlayingVoice]) -> ProcessParams<'a> {
    ProcessParams {
        trigger_stage: false,
        has_triggered_voices: false,
        samples: 1,
        sample_rate: SAMPLE_RATE,
        tempo: DEFAULT_TEMPO,
        needs_update_ui: false,
        smooth_params: SmoothedSampleParams::new(SAMPLE_RATE),
        active_voices: voices,
        last_voice_idx: Some(0),
    }
}

fn ctx<'a>(
    arena: &'a mut OutputsArena,
    audio_end: &'a mut AudioEnd,
    voices: &'a [PlayingVoice],
) -> ProcessContext<'a, AudioEnd> {
    ProcessContext {
        outputs_arena: arena,
        audio_end,
        params: process_params(voices),
    }
}

fn spectral_filter(
    filter_type: FilterType,
    spectrum_slot: usize,
    arena: &mut OutputsArena,
) -> SpectralFilter {
    let mut filter = SpectralFilter::from_config(&SpectralFilterConfig {
        id: 2,
        filter_type,
        linear_phase: true,
        keytrack: 1.0,
        q_cutoff: StereoSample::splat(Q_CUTOFF_OCTAVES),
        q_rolloff: StereoSample::splat(MIN_Q_ROLLOFF),
        cutoff: StereoSample::splat(CUTOFF_OCTAVES),
        resonance: StereoSample::splat(resonance_for_q(Q)),
        ..SpectralFilterConfig::default()
    });

    filter.set_input_slots(&[InputSlots::Spectral {
        input_type: Input::Spectrum,
        slot: spectrum_slot,
    }]);
    arena.allocate_slot(&mut filter);

    filter
}

fn bench_spectral_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("synth_engine/spectral_filter");
    group.warm_up_time(std::time::Duration::from_secs(1));
    group.measurement_time(std::time::Duration::from_secs(3));
    group.throughput(Throughput::Elements(
        (SPECTRAL_BUFFER_SIZE * NUM_CHANNELS * VOICES) as u64,
    ));

    let voices = voices();
    let mut arena = OutputsArena::new(BUFFER_SIZE);
    let mut spectrum = Spectral::new(1, &sawtooth());
    let mut audio_end = AudioEnd;

    arena.allocate_slot(&mut spectrum);
    spectrum.process(&mut ctx(&mut arena, &mut audio_end, &voices));

    let spectrum_slot = spectrum.output_slot();

    for filter_type in FilterType::ALL {
        let mut filter = spectral_filter(filter_type, spectrum_slot, &mut arena);

        group.bench_function(
            BenchmarkId::from_parameter(format!("{filter_type:?}")),
            |b| {
                b.iter(|| {
                    let mut ctx = ctx(&mut arena, &mut audio_end, &voices);

                    filter.process(black_box(&mut ctx));
                    black_box(&ctx);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_spectral_filter);
criterion_main!(benches);
