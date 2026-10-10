use std::hint::black_box;

use addi_engine::{
    EngineConfig, EngineParams, Input, LinkConfig, MAX_BLOCK_SIZE, ModuleConfig, ModuleId,
    NUM_CHANNELS, Note, OUTPUT_MODULE_ID, Sample, StereoSample, SynthEngine,
    harmonic_editor::HarmonicEditorConfig,
    oscillator::{MAX_UNISON_VOICES, OscillatorConfig},
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const SAMPLE_RATE: Sample = 48_000.0;
const HARMONIC_EDITOR_ID: ModuleId = 1;
const OSCILLATOR_ID: ModuleId = 2;

fn minimal_engine_config(engine: EngineParams, osc: OscillatorConfig) -> EngineConfig {
    EngineConfig {
        engine,
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HARMONIC_EDITOR_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(osc)),
        ],
        links: vec![
            LinkConfig::direct(HARMONIC_EDITOR_ID, OSCILLATOR_ID, Input::Spectrum),
            LinkConfig::direct(OSCILLATOR_ID, OUTPUT_MODULE_ID, Input::Audio),
        ],
    }
}

fn make_engine(engine: EngineParams, osc: OscillatorConfig) -> SynthEngine {
    let config = minimal_engine_config(engine, osc);

    SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine config")
}

fn trigger_notes(engine: &mut SynthEngine, count: usize) {
    trigger_notes_at(engine, count, 60);
}

fn trigger_notes_at(engine: &mut SynthEngine, count: usize, base_note: u8) {
    for i in 0..count {
        engine.handle_note_on(
            Note {
                channel: 0,
                note: base_note + i as u8,
                velocity: 1.0,
                host_id: None,
            },
            0,
        );
    }
}

fn process_block(engine: &mut SynthEngine, samples: usize) -> [Sample; MAX_BLOCK_SIZE] {
    let mut left = [0.0; MAX_BLOCK_SIZE];
    let mut right = [0.0; MAX_BLOCK_SIZE];

    let mut terminated = Vec::new();
    engine.process(
        samples,
        false,
        &mut terminated,
        [&mut left[..samples], &mut right[..samples]],
    );

    left
}

fn bench_process(c: &mut Criterion) {
    let mut group = c.benchmark_group("synth_engine/oscillator_path");

    {
        let mut engine = make_engine(
            EngineParams::default(),
            OscillatorConfig {
                id: OSCILLATOR_ID,
                unison_voices: 1,
                ..OscillatorConfig::default()
            },
        );
        trigger_notes(&mut engine, 1);

        let samples = MAX_BLOCK_SIZE;
        group.throughput(Throughput::Elements((samples * NUM_CHANNELS) as u64));
        group.bench_function("unison/1", |b| {
            b.iter(|| black_box(process_block(&mut engine, samples)));
        });
    }

    // One voice, 16 unison. At 48 kHz the inverse FFT is half above ~47 Hz
    // (C2) and full below that (C1).
    for (name, note) in [("half", 36u8), ("full", 24)] {
        let mut engine = make_engine(
            EngineParams::default(),
            OscillatorConfig {
                id: OSCILLATOR_ID,
                unison_voices: MAX_UNISON_VOICES,
                ..OscillatorConfig::default()
            },
        );
        trigger_notes_at(&mut engine, 1, note);

        let samples = MAX_BLOCK_SIZE;
        group.throughput(Throughput::Elements((samples * NUM_CHANNELS) as u64));
        group.bench_function(BenchmarkId::new("unison16", name), |b| {
            b.iter(|| black_box(process_block(&mut engine, samples)));
        });
    }

    for voice_count in [1, 4, 8, 16] {
        let mut engine = make_engine(
            EngineParams {
                num_voices: voice_count,
                ..EngineParams::default()
            },
            OscillatorConfig {
                id: OSCILLATOR_ID,
                unison_voices: 4,
                ..OscillatorConfig::default()
            },
        );
        trigger_notes(&mut engine, voice_count);

        let samples = MAX_BLOCK_SIZE;
        group.throughput(Throughput::Elements(
            (samples * NUM_CHANNELS * voice_count) as u64,
        ));
        group.bench_with_input(
            BenchmarkId::new("voices", voice_count),
            &voice_count,
            |b, _| {
                b.iter(|| black_box(process_block(&mut engine, samples)));
            },
        );
    }

    for samples in [8, 32, 64, MAX_BLOCK_SIZE] {
        let mut engine = make_engine(
            EngineParams {
                block_size: samples,
                ..EngineParams::default()
            },
            OscillatorConfig {
                id: OSCILLATOR_ID,
                unison_voices: 4,
                ..OscillatorConfig::default()
            },
        );
        trigger_notes(&mut engine, 1);

        group.throughput(Throughput::Elements((samples * NUM_CHANNELS) as u64));
        group.bench_with_input(BenchmarkId::new("block_size", samples), &samples, |b, _| {
            b.iter(|| black_box(process_block(&mut engine, samples)));
        });
    }

    {
        let mut engine = make_engine(
            EngineParams {
                num_voices: 16,
                ..EngineParams::default()
            },
            OscillatorConfig {
                id: OSCILLATOR_ID,
                unison_voices: MAX_UNISON_VOICES,
                detune: StereoSample::splat(0.05),
                ..OscillatorConfig::default()
            },
        );
        // MIDI 15..=30 stay on the full table at 48 kHz (half begins at
        // note 31, ~49 Hz). Same-note repeats are ignored, so the notes
        // must be distinct to occupy all 16 voices.
        trigger_notes_at(&mut engine, 16, 15);

        let samples = MAX_BLOCK_SIZE;
        group.throughput(Throughput::Elements((samples * NUM_CHANNELS * 16) as u64));
        group.bench_function("heavy_patch", |b| {
            b.iter(|| black_box(process_block(&mut engine, samples)));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_process);
criterion_main!(benches);
