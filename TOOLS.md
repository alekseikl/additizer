# TOOLS.md

Commands and workflows for benchmarks and test coverage in this repository.

## Benchmarks

Performance benchmarks use [Criterion](https://github.com/bheisler/criterion.rs).
`crates/addi-engine/benches/synth_engine.rs` exercises the full `SynthEngine::process` path (same as the
audio thread). `crates/addi-dsp/benches/svf.rs` and `crates/addi-dsp/benches/spectral_filter.rs` measure the filter
implementations on their own.

**Patch under test:** HarmonicEditor → Oscillator → Output (minimum config; `Output` is
added automatically by `SynthEngine::try_new`).

`cargo bench` uses the **`bench` profile** (inherits from `release`, so results are
optimized).

```shell
# Run all benchmarks
cargo bench --workspace

# Run one target
cargo bench -p addi-engine --bench synth_engine
cargo bench -p addi-dsp --bench svf
cargo bench -p addi-dsp --bench spectral_filter

# Run a single scenario (Criterion filter is a regex on the benchmark id)
cargo bench -p addi-engine --bench synth_engine -- heavy_patch
cargo bench -p addi-engine --bench synth_engine -- 'unison/16'
cargo bench -p addi-dsp --bench svf -- LowPass24
cargo bench -p addi-dsp --bench spectral_filter -- 'linear_phase/LowPass24'

# Quick iteration while tuning (fewer samples, short warm-up)
cargo bench -p addi-engine --bench synth_engine -- heavy_patch --sample-size 10 --warm-up-time 0.1
```

HTML reports are written to `target/criterion/`.

**Scenarios** (group `synth_engine/oscillator_path`):

| Benchmark | What it varies |
|-----------|----------------|
| `unison/1,4,8,16` | Unison voice count (single note, 128-sample block) |
| `voices/1,4,8,16` | Polyphony (4 unison voices) |
| `block_size/8,32,64,128` | Engine block size (4 unison, single note) |
| `heavy_patch` | 16 voices (MIDI 15–30) × 16 unison with detune, all on the full table |

Throughput for `synth_engine` is reported in stereo output samples per second
(`samples × channels`, and `× voices` where applicable).

**Oscillator render loop** (group `synth_engine/oscillator_render`): one voice, 16 unison
voices with detune and unison phase/gain blends, to measure the per-sample cost of
`Oscillator::render_voice_samples` in isolation from polyphony.

| Benchmark | What it varies |
|-----------|----------------|
| `single_voice_unison16/note_pitch` | Constant note pitch (no `Pitch` input) |
| `single_voice_unison16/pitch_input` | `Pitch` module → oscillator pitch input, as in the default patch |
| `single_voice_unison16/size_crossfade` | Legato C1↔C2, so each block crossfades full and half tables (`accumulate_unison_lane`). Time is both directions |

Throughput here is unison-voice samples per second (`samples × channels × 16`).

```shell
cargo bench -p addi-engine --bench synth_engine -- oscillator_render
```

**Filter scenarios:**

| Benchmark | What it measures |
|-----------|------------------|
| `svf/<type>` | One channel, 128-sample block, cutoff +2 octaves from C4, Q = 1 |
| `spectral_filter/linear_phase/<type>` | 1024-bin spectrum, `apply_response`, magnitude-only |

SVF throughput is samples per second for that channel. Spectral throughput is bins per second.

## Test coverage

Coverage uses [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) (LLVM
instrumentation). Install once:

```shell
cargo install cargo-llvm-cov
```

```shell
# Terminal summary
cargo llvm-cov

# HTML report
cargo llvm-cov --html
open target/llvm-cov/html/index.html

# LCOV output (CI / IDE integration)
cargo llvm-cov --lcov --output-path lcov.info
```

Useful options:

```shell
# Run a subset of tests
cargo llvm-cov -- oscillator

# Exclude test modules from the report
cargo llvm-cov --ignore-filename-regex 'tests\.rs'
```
