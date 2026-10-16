//! `Oscillator::process` for one voice at 1, 4, and 16 unison.
//!
//! A spectral module writes a harmonic series. A control module writes a
//! one-semitone pitch ramp. Both run outside the timer. `half_fft` is
//! triggered from C4 and stays on the half table. `full_fft` is triggered
//! from C1 and stays on the full table. Every active voice shares that ramp.

use std::hint::black_box;

use addi_engine::{
    ComplexSample, NUM_CHANNELS, Oscillator, PlayingVoice, SPECTRAL_BUFFER_SIZE,
    SmoothedSampleParams, Smp, SpectralBuffer, StereoSample, note_to_pitch,
    oscillator::{MAX_UNISON_VOICES, OscillatorConfig},
    routing::{Input, InputSlots, OutputsArena, ProcessContext, ProcessParams},
    stub::AudioEnd,
    synth_module::SynthModule,
};
use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};

#[path = "aux/mod.rs"]
mod aux;

use aux::{Control, Spectral};

/// 256-sample block at 48 kHz.
const BLOCK: usize = 256;
const BUFFER_SIZE: usize = BLOCK + 1;
const SAMPLE_RATE: Smp = 48_000.0;

/// C4. A one-semitone ramp from here stays on the half table at 48 kHz.
const HALF_NOTE: u8 = 60;
/// C1. A one-semitone ramp from here stays on the full table at 48 kHz.
const FULL_NOTE: u8 = 24;

/// Harmonic series, amplitude 1/n, across the whole spectrum. Table size then
/// follows pitch, because the spectrum does not cap the harmonic cutoff.
fn spectrum() -> SpectralBuffer {
    let mut spectrum = [ComplexSample::ZERO; SPECTRAL_BUFFER_SIZE];

    for (harmonic, bin) in spectrum.iter_mut().enumerate().skip(1) {
        *bin = ComplexSample::new(1.0 / harmonic as Smp, 0.0);
    }

    spectrum
}

fn pitch_ramp(from: Smp, to: Smp) -> [Smp; BLOCK] {
    let steps = (BLOCK - 1) as Smp;

    std::array::from_fn(|i| from + (to - from) * (i as Smp / steps))
}

struct Case {
    id: &'static str,
    voices: usize,
    unison: usize,
}

const CASES: &[Case] = &[
    Case {
        id: "voices1_unison1",
        voices: 1,
        unison: 1,
    },
    Case {
        id: "voices1_unison4",
        voices: 1,
        unison: 4,
    },
    Case {
        id: "single_voice_unison16",
        voices: 1,
        unison: MAX_UNISON_VOICES,
    },
];

struct Rig {
    arena: OutputsArena,
    pitch: Control,
    osc: Oscillator,
    audio_end: AudioEnd,
    voices: Vec<PlayingVoice>,
}

fn ctx<'a>(
    arena: &'a mut OutputsArena,
    audio_end: &'a mut AudioEnd,
    voices: &'a [PlayingVoice],
    trigger_stage: bool,
) -> ProcessContext<'a, AudioEnd> {
    let last_voice_idx = voices.last().map(|voice| voice.voice_idx());

    ProcessContext {
        outputs_arena: arena,
        audio_end,
        params: ProcessParams {
            trigger_stage,
            has_triggered_voices: false,
            samples: BLOCK,
            sample_rate: SAMPLE_RATE,
            needs_update_ui: false,
            smooth_params: SmoothedSampleParams::new(SAMPLE_RATE),
            active_voices: voices,
            last_voice_idx,
        },
    }
}

impl Rig {
    fn new(voices: usize, unison: usize) -> Self {
        let mut arena = OutputsArena::new(BUFFER_SIZE);
        let mut spectrum = Spectral::new(1, &spectrum());
        let mut pitch = Control::constant(3, 0.0);
        let mut audio_end = AudioEnd;
        let voices = (0..voices)
            .map(|voice_idx| PlayingVoice::new(voice_idx as u8, HALF_NOTE))
            .collect::<Vec<_>>();

        arena.allocate_slot(&mut spectrum);
        arena.allocate_slot(&mut pitch);

        spectrum.process(&mut ctx(&mut arena, &mut audio_end, &voices, false));

        let mut osc = Oscillator::from_config(&osc_config(unison), BUFFER_SIZE);
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
            voices,
        }
    }

    fn render_pitch(&mut self) {
        self.pitch.process(&mut ctx(
            &mut self.arena,
            &mut self.audio_end,
            &self.voices,
            false,
        ));
    }

    /// Writes a one-semitone ramp starting at `note` into every active voice.
    ///
    /// Control output is one sample ahead, so sample 0 stays at the previous
    /// ramp's end until the next pass.
    fn set_pitch_ramp(&mut self, note: u8) {
        for voice in &mut self.voices {
            *voice = PlayingVoice::new(voice.voice_idx() as u8, note);
        }

        let from = note_to_pitch(note as Smp);
        let to = note_to_pitch((note + 1) as Smp);

        self.pitch.set_samples(&pitch_ramp(from, to));
        self.render_pitch();
    }

    /// Two control passes fill the whole block with the ramp. The trigger pass
    /// latches that ramp's table size, and the oscillator pass installs the wave.
    fn settle(&mut self, note: u8) {
        self.set_pitch_ramp(note);
        self.set_pitch_ramp(note);
        self.process_trigger();
        self.process();
    }

    fn process_trigger(&mut self) {
        let mut ctx = ctx(&mut self.arena, &mut self.audio_end, &self.voices, true);

        self.osc.process(black_box(&mut ctx));
        black_box(ctx);
    }

    fn process(&mut self) {
        let mut ctx = ctx(&mut self.arena, &mut self.audio_end, &self.voices, false);

        self.osc.process(black_box(&mut ctx));
        black_box(ctx);
    }
}

fn osc_config(unison: usize) -> OscillatorConfig {
    OscillatorConfig {
        id: 2,
        unison_voices: unison,
        detune: StereoSample::splat(0.05),
        detune_focus: StereoSample::splat(0.3),
        phases_blend: StereoSample::splat(0.5),
        gains_blend: StereoSample::splat(0.5),
        ..OscillatorConfig::default()
    }
}

fn elements(case: &Case) -> u64 {
    (BLOCK * NUM_CHANNELS * case.voices * case.unison) as u64
}

fn bench_case(group: &mut BenchmarkGroup<WallTime>, case: &Case) {
    group.throughput(Throughput::Elements(elements(case)));

    {
        let mut rig = Rig::new(case.voices, case.unison);
        rig.settle(HALF_NOTE);

        group.bench_function(BenchmarkId::new(case.id, "half_fft"), |b| {
            b.iter(|| rig.process())
        });
    }

    {
        let mut rig = Rig::new(case.voices, case.unison);
        rig.settle(FULL_NOTE);

        group.bench_function(BenchmarkId::new(case.id, "full_fft"), |b| {
            b.iter(|| rig.process())
        });
    }
}

fn bench_oscillator(c: &mut Criterion) {
    let mut group = c.benchmark_group("oscillator");
    group.warm_up_time(std::time::Duration::from_secs(1));
    group.measurement_time(std::time::Duration::from_secs(3));

    for case in CASES {
        bench_case(&mut group, case);
    }

    group.finish();
}

criterion_group!(benches, bench_oscillator);
criterion_main!(benches);
