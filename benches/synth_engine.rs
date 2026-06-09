use std::{hint::black_box, sync::Arc};

use additizer::synth_engine::{
    EngineConfig, EngineParams, ExternalParamsBlock, Input, LinkConfig, MAX_BLOCK_SIZE,
    ModuleConfig, ModuleId, NUM_CHANNELS, OUTPUT_MODULE_ID, Sample, StereoSample, SynthEngine,
    harmonic_editor::HarmonicEditorConfig,
    oscillator::{MAX_UNISON_VOICES, OscillatorConfig},
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use nih_plug::prelude::*;

const SAMPLE_RATE: Sample = 48_000.0;
const HARMONIC_EDITOR_ID: ModuleId = 1;
const OSCILLATOR_ID: ModuleId = 2;

fn bench_deps() -> (Arc<FloatParam>, Arc<ExternalParamsBlock>) {
    let volume = Arc::new(FloatParam::new(
        "Volume",
        0.0,
        FloatRange::Linear { min: 0.0, max: 1.0 },
    ));

    let float_param = |name: &str| {
        Arc::new(FloatParam::new(
            name,
            0.0,
            FloatRange::Linear { min: 0.0, max: 1.0 },
        ))
    };

    let external_params = Arc::new(ExternalParamsBlock {
        float_params: [
            float_param("Float Param 1"),
            float_param("Float Param 2"),
            float_param("Float Param 3"),
            float_param("Float Param 4"),
        ],
    });

    (volume, external_params)
}

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
            LinkConfig {
                src_id: HARMONIC_EDITOR_ID,
                dst_id: OSCILLATOR_ID,
                dst_input: Input::Spectrum,
                amount: StereoSample::ONE,
                modulator_id: None,
            },
            LinkConfig {
                src_id: OSCILLATOR_ID,
                dst_id: OUTPUT_MODULE_ID,
                dst_input: Input::Audio,
                amount: StereoSample::ONE,
                modulator_id: None,
            },
        ],
    }
}

fn make_engine(engine: EngineParams, osc: OscillatorConfig) -> SynthEngine {
    let (volume, external_params) = bench_deps();
    let config = minimal_engine_config(engine, osc);

    SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("valid engine config")
}

fn trigger_notes(engine: &mut SynthEngine, count: usize) {
    for i in 0..count {
        engine.handle_note_on(0, (60 + i) as u8, 1.0);
    }
}

fn process_block(engine: &mut SynthEngine, samples: usize) -> [Sample; MAX_BLOCK_SIZE] {
    let mut left = [0.0; MAX_BLOCK_SIZE];
    let mut right = [0.0; MAX_BLOCK_SIZE];

    engine.process(
        samples,
        false,
        [&mut left[..samples], &mut right[..samples]].into_iter(),
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
                stereo_spectrum: false,
                ..EngineParams::default()
            },
            OscillatorConfig {
                id: OSCILLATOR_ID,
                ..OscillatorConfig::default()
            },
        );
        trigger_notes(&mut engine, 1);

        let samples = MAX_BLOCK_SIZE;
        group.throughput(Throughput::Elements((samples * NUM_CHANNELS) as u64));
        group.bench_function("mono_spectrum", |b| {
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

criterion_group!(benches, bench_process);
criterion_main!(benches);
