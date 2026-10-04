use std::hint::black_box;

use addi_engine::{
    EngineConfig, EngineParams, Input, LinkConfig, MAX_BLOCK_SIZE, ModuleConfig, ModuleId,
    NUM_CHANNELS, Note, OUTPUT_MODULE_ID, Sample, StereoSample, SynthEngine,
    harmonic_editor::HarmonicEditorConfig,
    oscillator::{MAX_UNISON_VOICES, OscillatorConfig},
    pitch::PitchConfig,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const SAMPLE_RATE: Sample = 48_000.0;
const HARMONIC_EDITOR_ID: ModuleId = 1;
const OSCILLATOR_ID: ModuleId = 2;
const PITCH_ID: ModuleId = 3;

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

/// Like `minimal_engine_config`, plus a `Pitch` module driving the
/// oscillator's pitch input (as in the default patch), so the oscillator takes
/// its per-sample pitch path instead of the constant note pitch.
fn pitched_engine_config(engine: EngineParams, osc: OscillatorConfig) -> EngineConfig {
    let mut config = minimal_engine_config(engine, osc);

    config
        .modules
        .push(ModuleConfig::Pitch(Box::new(PitchConfig {
            id: PITCH_ID,
            ..PitchConfig::default()
        })));
    config
        .links
        .push(LinkConfig::direct(PITCH_ID, OSCILLATOR_ID, Input::Pitch));
    config
}

fn make_engine(engine: EngineParams, osc: OscillatorConfig) -> SynthEngine {
    let config = minimal_engine_config(engine, osc);

    SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine config")
}

fn trigger_notes(engine: &mut SynthEngine, count: usize) {
    for i in 0..count {
        engine.handle_note_on(
            Note {
                channel: 0,
                note: (60 + i) as u8,
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

    for unison in [1, 4, 8, MAX_UNISON_VOICES] {
        let mut engine = make_engine(
            EngineParams::default(),
            OscillatorConfig {
                id: OSCILLATOR_ID,
                unison_voices: unison,
                ..OscillatorConfig::default()
            },
        );
        trigger_notes(&mut engine, 1);

        let samples = MAX_BLOCK_SIZE;
        group.throughput(Throughput::Elements((samples * NUM_CHANNELS) as u64));
        group.bench_with_input(BenchmarkId::new("unison", unison), &unison, |b, _| {
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
        trigger_notes(&mut engine, 16);

        let samples = MAX_BLOCK_SIZE;
        group.throughput(Throughput::Elements((samples * NUM_CHANNELS * 16) as u64));
        group.bench_function("heavy_patch", |b| {
            b.iter(|| black_box(process_block(&mut engine, samples)));
        });
    }

    group.finish();
}

/// Per-sample cost of the oscillator render loop: one voice, max unison, with
/// detune, unison phase/gain blends and a pitch input so every per-sample code
/// path in `render_voice_samples` is exercised.
fn bench_oscillator_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("synth_engine/oscillator_render");

    let osc = OscillatorConfig {
        id: OSCILLATOR_ID,
        unison_voices: MAX_UNISON_VOICES,
        detune: StereoSample::splat(0.05),
        detune_power: StereoSample::splat(0.3),
        phases_blend: StereoSample::splat(0.5),
        gains_blend: StereoSample::splat(0.5),
        ..OscillatorConfig::default()
    };

    for (name, pitched) in [("note_pitch", false), ("pitch_input", true)] {
        let config = if pitched {
            pitched_engine_config(EngineParams::default(), osc.clone())
        } else {
            minimal_engine_config(EngineParams::default(), osc.clone())
        };
        let mut engine = SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine config");
        trigger_notes(&mut engine, 1);

        let samples = MAX_BLOCK_SIZE;
        group.throughput(Throughput::Elements(
            (samples * NUM_CHANNELS * MAX_UNISON_VOICES) as u64,
        ));
        group.bench_function(BenchmarkId::new("single_voice_unison16", name), |b| {
            b.iter(|| black_box(process_block(&mut engine, samples)));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_process, bench_oscillator_render);
criterion_main!(benches);
