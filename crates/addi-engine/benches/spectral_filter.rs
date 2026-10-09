//! `SpectralFilter::process` for every filter type at 16 voices.
//!
//! Cutoff is +2 octaves, resonant-stage Q is 1, Q-cutoff is 8 octaves, and
//! Q-rolloff is the module minimum, matching `addi-dsp`'s `spectral_filter`
//! bench. Linear phase matches that bench too. Voices sit on C4, so key
//! tracking adds no cutoff offset. A full-length sawtooth spectrum is rendered
//! once; only `SpectralFilter::process` is timed.

use std::hint::black_box;

use addi_dsp::filters::{
    control::{MAX_RESONANCE_Q, ZERO_RESONANCE_Q},
    spectral_filter::FilterType,
};
use addi_engine::{
    HarmonicEditor, MAX_BANDWIDTH, ModuleHandle, NUM_CHANNELS, PlayingVoice, SPECTRAL_BUFFER_SIZE,
    Sample, SmoothedSampleParams, SpectralFilter, StereoSample,
    harmonic_editor::HarmonicEditorConfig,
    routing::{Input, InputSlots, OutputsArena, ProcessContext, ProcessParams},
    spectral_filter::{MIN_Q_ROLLOFF, SpectralFilterConfig},
    stub::{AudioEnd, StubLinks},
    synth_module::SynthModule,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const SAMPLE_RATE: Sample = 48_000.0;

const VOICES: usize = 16;

/// Octaves of the note fundamental. Two octaves up is the 4th harmonic.
const CUTOFF_OCTAVES: Sample = 2.0;

/// Resonant-stage Q, matching `crates/addi-dsp/benches/spectral_filter.rs`.
const Q: Sample = 1.0;

/// Octaves. At and above this, Q is not reduced.
const Q_CUTOFF_OCTAVES: Sample = 8.0;

fn resonance_for_q(q: Sample) -> Sample {
    ((q - ZERO_RESONANCE_Q) / (MAX_RESONANCE_Q - ZERO_RESONANCE_Q)).cbrt()
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
        needs_update_ui: false,
        smooth_params: SmoothedSampleParams::new(SAMPLE_RATE),
        active_voices: voices,
    }
}

/// Renders a full-length sawtooth into an arena slot the filter can read.
fn render_spectrum(voices: &[PlayingVoice]) -> (OutputsArena, usize) {
    let editor = HarmonicEditor::from_config(&HarmonicEditorConfig {
        id: 1,
        // Fixed harmonic count so every voice writes `SPECTRAL_BUFFER_SIZE` bins.
        bandwidth: MAX_BANDWIDTH as i32,
        ..HarmonicEditorConfig::default()
    });
    let mut handle = ModuleHandle::<StubLinks>::HarmonicEditor(Box::new(editor));
    let mut arena = OutputsArena::new();
    arena.allocate_slot(&mut handle);

    let ModuleHandle::HarmonicEditor(editor) = handle else {
        unreachable!("harmonic editor handle");
    };
    let mut editor = *editor;
    let spectrum_slot = editor.output_slot();
    let mut audio_end = AudioEnd;

    editor.process(&mut ProcessContext {
        outputs_arena: &mut arena,
        audio_end: &mut audio_end,
        params: process_params(voices),
    });

    (arena, spectrum_slot)
}

fn spectral_filter(
    filter_type: FilterType,
    spectrum_slot: usize,
    arena: &mut OutputsArena,
) -> SpectralFilter {
    let filter = SpectralFilter::from_config(&SpectralFilterConfig {
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
    let mut handle = ModuleHandle::<StubLinks>::SpectralFilter(Box::new(filter));
    arena.allocate_slot(&mut handle);

    let ModuleHandle::SpectralFilter(filter) = handle else {
        unreachable!("spectral filter handle");
    };
    let mut filter = *filter;

    filter.set_input_slots(&[InputSlots::Spectral {
        input_type: Input::Spectrum,
        slot: spectrum_slot,
    }]);

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
    let (mut arena, spectrum_slot) = render_spectrum(&voices);

    for filter_type in FilterType::ALL {
        let mut filter = spectral_filter(filter_type, spectrum_slot, &mut arena);
        let mut audio_end = AudioEnd;

        group.bench_function(
            BenchmarkId::from_parameter(format!("{filter_type:?}")),
            |b| {
                b.iter(|| {
                    let mut ctx = ProcessContext {
                        outputs_arena: &mut arena,
                        audio_end: &mut audio_end,
                        params: process_params(&voices),
                    };

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
