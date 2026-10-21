use std::hint::black_box;

use addi_engine::{
    DEFAULT_TEMPO, Envelope, ModuleHandle, PlayingVoice, SmoothedSampleParams, Smp,
    envelope::{EnvelopeConfig, stub::Links},
    routing::{OutputsArena, ProcessContext, ProcessParams},
    stub::{AudioEnd, StubLinks},
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// 48 kHz host block.
const BLOCK: usize = 128;
const BUFFER_SIZE: usize = BLOCK + 1;

const SAMPLE_RATE: Smp = 48_000.0;

fn envelope(slope: Smp) -> (Envelope<Links>, OutputsArena) {
    let envelope = Envelope::<Links>::from_config(&EnvelopeConfig {
        id: 1,
        // Stays inside the attack for the whole measurement.
        attack: 1.0e9.into(),
        attack_slope: slope,
        sustain: 1.0.into(),
        ..EnvelopeConfig::default()
    });
    let mut handle = ModuleHandle::<StubLinks>::Envelope(Box::new(envelope));
    let mut arena = OutputsArena::new(BUFFER_SIZE);
    arena.allocate_slot(&mut handle);

    let ModuleHandle::Envelope(envelope) = handle else {
        unreachable!("envelope handle");
    };

    (*envelope, arena)
}

fn bench_envelope(c: &mut Criterion) {
    let mut group = c.benchmark_group("envelope");
    group.warm_up_time(std::time::Duration::from_secs(1));
    group.measurement_time(std::time::Duration::from_secs(3));

    for (len, slope) in [
        (BLOCK, 0.0),
        (BLOCK, 0.3),
        (BLOCK, 1.0),
        (BLOCK, -1.0),
        (127, 1.0),
    ] {
        let (mut envelope, mut arena) = envelope(slope);
        let mut audio_end = AudioEnd;
        let voices = [PlayingVoice::new(0, 60)];

        group.throughput(Throughput::Elements(len as u64));
        group.bench_with_input(
            BenchmarkId::new("curve", format!("n={len}/slope={slope}")),
            &slope,
            |b, _| {
                b.iter(|| {
                    let mut ctx = ProcessContext {
                        outputs_arena: &mut arena,
                        audio_end: &mut audio_end,
                        params: ProcessParams {
                            trigger_stage: false,
                            has_triggered_voices: false,
                            samples: len,
                            sample_rate: SAMPLE_RATE,
                            tempo: DEFAULT_TEMPO,
                            needs_update_ui: false,
                            smooth_params: SmoothedSampleParams::new(SAMPLE_RATE),
                            active_voices: &voices,
                            last_voice_idx: Some(0),
                        },
                    };

                    envelope.process(black_box(&mut ctx));
                    black_box(&ctx);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_envelope);
criterion_main!(benches);
