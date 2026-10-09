//! `Svf::process` for every filter type at 16 voices.
//!
//! Cutoff is +2 octaves from C4 and the resonant stage Q is 1, matching
//! `addi-dsp`'s `svf` bench. Resonance and drive stay constant, so the module
//! takes the stationary parameter path. A rising envelope is rendered into the
//! audio input once per type; only `Svf::process` is timed.

use std::hint::black_box;

use addi_dsp::filters::{
    control::{MAX_RESONANCE_Q, ZERO_RESONANCE_Q},
    svf::SvfType,
};
use addi_engine::{
    Envelope, ModuleHandle, NUM_CHANNELS, PlayingVoice, Sample, SmoothedSampleParams, StereoSample,
    Svf,
    envelope::{EnvelopeConfig, stub::Links as EnvelopeLinks},
    from_ms,
    routing::{Input, InputSlots, OutputsArena, ProcessContext, ProcessParams},
    stub::{AudioEnd, StubLinks},
    svf::SvfConfig,
    synth_module::SynthModule,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// Matches `addi_engine::MAX_BLOCK_SIZE`.
const BLOCK: usize = 128;

const SAMPLE_RATE: Sample = 48_000.0;

const VOICES: usize = 16;

/// Octaves relative to C4. Two octaves up is about 1 kHz.
const CUTOFF_OCTAVES: Sample = 2.0;

/// Resonant-stage Q, matching `crates/addi-dsp/benches/svf.rs`.
const Q: Sample = 1.0;

fn resonance_for_q(q: Sample) -> Sample {
    ((q - ZERO_RESONANCE_Q) / (MAX_RESONANCE_Q - ZERO_RESONANCE_Q)).cbrt()
}

fn voices() -> [PlayingVoice; VOICES] {
    std::array::from_fn(|i| PlayingVoice::new(i as u8, 60))
}

fn process_params<'a>(
    voices: &'a [PlayingVoice],
    smooth_params: SmoothedSampleParams,
) -> ProcessParams<'a> {
    ProcessParams {
        trigger_stage: false,
        has_triggered_voices: false,
        samples: BLOCK,
        sample_rate: SAMPLE_RATE,
        needs_update_ui: false,
        smooth_params,
        active_voices: voices,
    }
}

/// Renders a rising ramp into an arena slot the filter can read as audio.
fn render_audio_input(voices: &[PlayingVoice]) -> (OutputsArena, usize) {
    let envelope = Envelope::<EnvelopeLinks>::from_config(&EnvelopeConfig {
        id: 1,
        // Longer than one block, so the whole block stays on the attack ramp.
        attack: from_ms(10.0).into(),
        attack_slope: 0.0,
        sustain: 1.0.into(),
        ..EnvelopeConfig::default()
    });
    let mut handle = ModuleHandle::<StubLinks>::Envelope(Box::new(envelope));
    let mut arena = OutputsArena::new();
    arena.allocate_slot(&mut handle);

    let ModuleHandle::Envelope(envelope) = handle else {
        unreachable!("envelope handle");
    };
    let mut envelope = *envelope;
    let audio_slot = envelope.output_slot();
    let mut audio_end = AudioEnd;
    let smooth_params = SmoothedSampleParams::new(SAMPLE_RATE);

    envelope.process(&mut ProcessContext {
        outputs_arena: &mut arena,
        audio_end: &mut audio_end,
        params: process_params(voices, smooth_params),
    });

    (arena, audio_slot)
}

fn svf(filter_type: SvfType, audio_slot: usize, arena: &mut OutputsArena) -> Svf {
    let svf = Svf::from_config(&SvfConfig {
        id: 2,
        filter_type,
        cutoff: StereoSample::splat(CUTOFF_OCTAVES),
        resonance: StereoSample::splat(resonance_for_q(Q)),
        ..SvfConfig::default()
    });
    let mut handle = ModuleHandle::<StubLinks>::Svf(Box::new(svf));
    arena.allocate_slot(&mut handle);

    let ModuleHandle::Svf(svf) = handle else {
        unreachable!("svf handle");
    };
    let mut svf = *svf;

    svf.set_input_slots(&[InputSlots::Direct {
        input_type: Input::Audio,
        slot: audio_slot,
    }]);

    svf
}

fn bench_svf(c: &mut Criterion) {
    let mut group = c.benchmark_group("synth_engine/svf");
    group.warm_up_time(std::time::Duration::from_secs(1));
    group.measurement_time(std::time::Duration::from_secs(3));
    group.throughput(Throughput::Elements((BLOCK * NUM_CHANNELS * VOICES) as u64));

    let voices = voices();
    let smooth_params = SmoothedSampleParams::new(SAMPLE_RATE);

    for filter_type in SvfType::ALL {
        let (mut arena, audio_slot) = render_audio_input(&voices);
        let mut svf = svf(filter_type, audio_slot, &mut arena);
        let mut audio_end = AudioEnd;

        group.bench_function(
            BenchmarkId::from_parameter(format!("{filter_type:?}")),
            |b| {
                b.iter(|| {
                    let mut ctx = ProcessContext {
                        outputs_arena: &mut arena,
                        audio_end: &mut audio_end,
                        params: process_params(&voices, smooth_params),
                    };

                    svf.process(black_box(&mut ctx));
                    black_box(&ctx);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_svf);
criterion_main!(benches);
