//! `Svf::process` for every filter type at 16 voices.
//!
//! Cutoff is +2 octaves from C4 and the resonant stage Q is 1, matching
//! `addi-dsp`'s `svf` bench. Resonance and drive stay constant, so the module
//! takes the stationary parameter path. An auxiliary audio module writes a
//! 10 ms linear ramp once; only `Svf::process` is timed.

use std::hint::black_box;

use addi_dsp::filters::{
    control::{MAX_RESONANCE_Q, ZERO_RESONANCE_Q},
    svf::SvfType,
};
use addi_engine::{
    NUM_CHANNELS, PlayingVoice, Sample, SmoothedSampleParams, StereoSample, Svf,
    routing::{Input, InputSlots, OutputsArena, ProcessContext, ProcessParams},
    stub::AudioEnd,
    svf::SvfConfig,
    synth_module::SynthModule,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[path = "aux/mod.rs"]
mod aux;

use aux::Audio;

/// Matches `addi_engine::MAX_BLOCK_SIZE`.
const BLOCK: usize = 128;

const SAMPLE_RATE: Sample = 48_000.0;

const VOICES: usize = 16;

/// Octaves relative to C4. Two octaves up is about 1 kHz.
const CUTOFF_OCTAVES: Sample = 2.0;

/// Resonant-stage Q, matching `crates/addi-dsp/benches/svf.rs`.
const Q: Sample = 1.0;

/// Seconds. The block is the start of this attack.
const ATTACK: Sample = 0.01;

fn resonance_for_q(q: Sample) -> Sample {
    ((q - ZERO_RESONANCE_Q) / (MAX_RESONANCE_Q - ZERO_RESONANCE_Q)).cbrt()
}

/// Linear rise from silence over [`ATTACK`].
fn rising_ramp() -> [Sample; BLOCK] {
    let slope = SAMPLE_RATE.recip() / ATTACK;

    std::array::from_fn(|i| slope * i as Sample)
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
        last_voice_idx: Some(0),
    }
}

fn ctx<'a>(
    arena: &'a mut OutputsArena,
    audio_end: &'a mut AudioEnd,
    voices: &'a [PlayingVoice],
    smooth_params: SmoothedSampleParams,
) -> ProcessContext<'a, AudioEnd> {
    ProcessContext {
        outputs_arena: arena,
        audio_end,
        params: process_params(voices, smooth_params),
    }
}

fn svf(filter_type: SvfType, audio_slot: usize, arena: &mut OutputsArena) -> Svf {
    let mut svf = Svf::from_config(&SvfConfig {
        id: 2,
        filter_type,
        cutoff: StereoSample::splat(CUTOFF_OCTAVES),
        resonance: StereoSample::splat(resonance_for_q(Q)),
        ..SvfConfig::default()
    });

    svf.set_input_slots(&[InputSlots::Direct {
        input_type: Input::Audio,
        slot: audio_slot,
    }]);
    arena.allocate_slot(&mut svf);

    svf
}

fn bench_svf(c: &mut Criterion) {
    let mut group = c.benchmark_group("synth_engine/svf");
    group.warm_up_time(std::time::Duration::from_secs(1));
    group.measurement_time(std::time::Duration::from_secs(3));
    group.throughput(Throughput::Elements((BLOCK * NUM_CHANNELS * VOICES) as u64));

    let voices = voices();
    let smooth_params = SmoothedSampleParams::new(SAMPLE_RATE);
    let mut arena = OutputsArena::new();
    let mut audio = Audio::constant(1, 0.0);
    let mut audio_end = AudioEnd;

    audio.set_samples(&rising_ramp());
    arena.allocate_slot(&mut audio);
    audio.process(&mut ctx(&mut arena, &mut audio_end, &voices, smooth_params));

    let audio_slot = audio.output_slot();

    for filter_type in SvfType::ALL {
        let mut svf = svf(filter_type, audio_slot, &mut arena);

        group.bench_function(
            BenchmarkId::from_parameter(format!("{filter_type:?}")),
            |b| {
                b.iter(|| {
                    let mut ctx = ctx(&mut arena, &mut audio_end, &voices, smooth_params);

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
