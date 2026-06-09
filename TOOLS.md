# TOOLS.md

Commands and workflows for benchmarks and test coverage in this repository.

## Benchmarks

Performance benchmarks use [Criterion](https://github.com/bheisler/criterion.rs) in
`benches/synth_engine.rs`. They exercise the full `SynthEngine::process` path (same as the
audio thread), not individual modules in isolation.

**Patch under test:** HarmonicEditor → Oscillator → Output (minimum config; `Output` is
added automatically by `SynthEngine::try_new`).

`cargo bench` uses the **`bench` profile** (inherits from `release`, so results are
optimized).

```shell
# Run all benchmarks
cargo bench --bench synth_engine

# Run a single scenario (Criterion filter is a regex on the benchmark id)
cargo bench --bench synth_engine -- heavy_patch
cargo bench --bench synth_engine -- 'unison/16'

# Quick iteration while tuning (fewer samples, skip warm-up)
cargo bench --bench synth_engine -- heavy_patch --sample-size 10 --warm-up-time 0
```

HTML reports are written to `target/criterion/`.

**Scenarios** (group `synth_engine/oscillator_path`):

| Benchmark | What it varies |
|-----------|----------------|
| `unison/1,4,8,16` | Unison voice count (single note, 128-sample block) |
| `voices/1,4,8,16` | Polyphony (4 unison voices) |
| `block_size/8,32,64,128` | Engine block size (4 unison, single note) |
| `mono_spectrum` | `stereo_spectrum: false` (shared waveform across channels) |
| `heavy_patch` | 16 voices × 16 unison with detune |

Throughput is reported in stereo output samples per second (`samples × channels`, and
`× voices` where applicable).

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
