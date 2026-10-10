//! `Oscillator::process` for one voice at 16 unison.
//!
//! A spectral module writes a harmonic series. A control module writes a
//! one-semitone pitch ramp. Both run outside the timer. `pitch_input` ramps
//! from C4 and stays on the half table. `full_fft` ramps from C1, so each
//! block rebuilds and reads the full table. `size_crossfade` alternates a C2
//! ramp (half table) and a C1 ramp (full table).

use std::hint::black_box;
use std::time::Instant;

use addi_engine::{
    ComplexSample, MAX_BLOCK_SIZE, NUM_CHANNELS, Oscillator, PlayingVoice, SPECTRAL_BUFFER_SIZE,
    Sample, SmoothedSampleParams, SpectralBuffer, StereoSample, note_to_pitch,
    oscillator::{MAX_UNISON_VOICES, OscillatorConfig},
    routing::{Input, InputSlots, OutputsArena, ProcessContext, ProcessParams},
    stub::AudioEnd,
    synth_module::SynthModule,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[path = "aux/mod.rs"]
mod aux;

use aux::{Control, Spectral};

const BLOCK: usize = MAX_BLOCK_SIZE;
const SAMPLE_RATE: Sample = 48_000.0;
const VOICE: usize = 0;

/// C4. A one-semitone ramp from here stays on the half table at 48 kHz.
const HALF_NOTE: u8 = 60;
/// C2. A one-semitone ramp from here stays on the half table at 48 kHz.
const UPPER_HALF_NOTE: u8 = 36;
/// C1. A one-semitone ramp from here stays on the full table at 48 kHz.
const FULL_NOTE: u8 = 24;

/// Harmonic series, amplitude 1/n, across the whole spectrum. Table size then
/// follows pitch, because the spectrum does not cap the harmonic cutoff.
fn spectrum() -> SpectralBuffer {
    let mut spectrum = [ComplexSample::ZERO; SPECTRAL_BUFFER_SIZE];

    for (harmonic, bin) in spectrum.iter_mut().enumerate().skip(1) {
        *bin = ComplexSample::new(1.0 / harmonic as Sample, 0.0);
    }

    spectrum
}

fn pitch_ramp(from: Sample, to: Sample) -> [Sample; BLOCK] {
    let steps = (BLOCK - 1) as Sample;

    std::array::from_fn(|i| from + (to - from) * (i as Sample / steps))
}

struct Rig {
    arena: OutputsArena,
    pitch: Control,
    osc: Oscillator,
    audio_end: AudioEnd,
    voice: PlayingVoice,
}

fn ctx<'a>(
    arena: &'a mut OutputsArena,
    audio_end: &'a mut AudioEnd,
    voices: &'a [PlayingVoice],
) -> ProcessContext<'a, AudioEnd> {
    ProcessContext {
        outputs_arena: arena,
        audio_end,
        params: ProcessParams {
            trigger_stage: false,
            has_triggered_voices: false,
            samples: BLOCK,
            sample_rate: SAMPLE_RATE,
            needs_update_ui: false,
            smooth_params: SmoothedSampleParams::new(SAMPLE_RATE),
            active_voices: voices,
        },
    }
}

impl Rig {
    fn new(config: OscillatorConfig) -> Self {
        let mut arena = OutputsArena::new();
        let mut spectrum = Spectral::new(1, &spectrum());
        let mut pitch = Control::constant(3, 0.0);
        let mut audio_end = AudioEnd;
        let voice = PlayingVoice::new(VOICE as u8, HALF_NOTE);

        arena.allocate_slot(&mut spectrum);
        arena.allocate_slot(&mut pitch);

        let voices = [voice];
        spectrum.process(&mut ctx(&mut arena, &mut audio_end, &voices));

        let mut osc = Oscillator::from_config(&config);
        osc.set_input_slots(&[
            InputSlots::Spectral {
                input_type: Input::Spectrum,
                slot: spectrum.output_slot(),
            },
            InputSlots::Direct {
                input_type: Input::Pitch,
                slot: pitch.output_slot(),
            },
        ]);
        arena.allocate_slot(&mut osc);

        Self {
            arena,
            pitch,
            osc,
            audio_end,
            voice,
        }
    }

    fn render_pitch(&mut self) {
        let voices = [self.voice];

        self.pitch
            .process(&mut ctx(&mut self.arena, &mut self.audio_end, &voices));
    }

    /// Writes a one-semitone ramp starting at `note`.
    ///
    /// Control output is one sample ahead, so sample 0 stays at the previous
    /// ramp's end until the next pass.
    fn set_pitch_ramp(&mut self, note: u8) {
        self.voice = PlayingVoice::new(VOICE as u8, note);
        let from = note_to_pitch(note as Sample);
        let to = note_to_pitch((note + 1) as Sample);

        self.pitch.set_samples(&pitch_ramp(from, to));
        self.render_pitch();
    }

    /// Two control passes fill the whole block with the ramp. The oscillator
    /// pass then installs the wavetable for that ramp's end.
    fn settle(&mut self, note: u8) {
        self.set_pitch_ramp(note);
        self.set_pitch_ramp(note);
        self.process();
    }

    fn process(&mut self) {
        let voices = [self.voice];
        let mut ctx = ctx(&mut self.arena, &mut self.audio_end, &voices);

        self.osc.process(black_box(&mut ctx));
        black_box(ctx);
    }
}

fn osc_config() -> OscillatorConfig {
    OscillatorConfig {
        id: 2,
        unison_voices: MAX_UNISON_VOICES,
        detune: StereoSample::splat(0.05),
        detune_focus: StereoSample::splat(0.3),
        phases_blend: StereoSample::splat(0.5),
        gains_blend: StereoSample::splat(0.5),
        ..OscillatorConfig::default()
    }
}

fn bench_oscillator(c: &mut Criterion) {
    let mut group = c.benchmark_group("oscillator");
    group.warm_up_time(std::time::Duration::from_secs(1));
    group.measurement_time(std::time::Duration::from_secs(3));
    group.throughput(Throughput::Elements(
        (BLOCK * NUM_CHANNELS * MAX_UNISON_VOICES) as u64,
    ));

    {
        let mut rig = Rig::new(osc_config());
        rig.settle(HALF_NOTE);

        group.bench_function(
            BenchmarkId::new("single_voice_unison16", "pitch_input"),
            |b| b.iter(|| rig.process()),
        );
    }

    {
        let mut rig = Rig::new(osc_config());
        rig.settle(FULL_NOTE);

        group.bench_function(BenchmarkId::new("single_voice_unison16", "full_fft"), |b| {
            b.iter(|| rig.process())
        });
    }

    // One iteration times both directions. Retuning happens outside the timer,
    // and each timed block crossfades the previous table size into the new one.
    {
        let mut rig = Rig::new(osc_config());
        rig.settle(FULL_NOTE);

        group.throughput(Throughput::Elements(
            (BLOCK * NUM_CHANNELS * MAX_UNISON_VOICES * 2) as u64,
        ));
        group.bench_function(
            BenchmarkId::new("single_voice_unison16", "size_crossfade"),
            |b| {
                b.iter_custom(|iters| {
                    let mut total = std::time::Duration::ZERO;

                    for _ in 0..iters {
                        rig.set_pitch_ramp(UPPER_HALF_NOTE);
                        let start = Instant::now();
                        rig.process();
                        total += start.elapsed();

                        rig.set_pitch_ramp(FULL_NOTE);
                        let start = Instant::now();
                        rig.process();
                        total += start.elapsed();
                    }

                    total
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_oscillator);
criterion_main!(benches);
