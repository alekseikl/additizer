# AGENTS.md

Guidance for AI agents working in this repository. Read this before making changes.

## Project overview

Additizer is a **modular synthesizer plugin** written in Rust. It builds as a
CLAP plugin (`cdylib`) and a standalone app, using the [`nih-plug`](https://github.com/robbert-vdh/nih-plug)
framework with an `egui`-based editor (`nih_plug_egui`, OpenGL via `baseview`).

The synth is a graph of **modules**. Some process audio in the time domain (oscillator,
mixer, amplifier, waveshaper, output), others operate on spectra (harmonic editor, spectral
filter/mixer/blend), and others are modulation sources (envelope, LFO, expressions/MPE,
external params). Modules are connected by **links** into a routing graph that is
topologically sorted and processed per voice/channel. See `README.md` for the user-facing
module list and `docs/module-routing-rules.md` for link/modulation rules.

## Commands

```shell
# Build the standalone app + CLAP bundle into ./target/bundled (requires cargo-nih-plug)
cargo nih-plug bundle additizer --release

# Run standalone, choosing a MIDI input device by name
cargo run --release -- --midi-input "Keystation Mini 32 MK3"
```

## Architecture

There are two threads that matter, and they must never block each other:

1. **Audio thread** — owns `SynthEngine` (`src/synth_engine.rs`). Real-time, allocation-free.
   `Additizer::process` (in `src/lib.rs`) splits the host buffer into blocks (≤ `MAX_BLOCK_SIZE`
   = 128, see `block_size()`), reorders note events, and drives the engine.
2. **UI thread** — owns `UiBridge` (`src/synth_engine/ui_bridge.rs`) and the `egui` editor
   (`src/editor.rs` + `src/editor/grid/`). Reads/writes engine state without touching the audio
   thread directly.

**`EngineFactory`** (`src/engine_factory.rs`) is the shared bridge between them. It holds the
live `SynthEngine` and `UiConfig` inside `ArcSwap<Mutex<…>>`. Loading a preset swaps in a brand
new engine; the audio thread detects the swap via `engine_changed()` and picks it up at the next
`process` call.

**Communication is lock-free.**

- UI → audio parameter / engine control changes go through `rtrb` ring buffers (`UiEnd` /
  `AudioEnd` link pairs), not by locking the engine during audio processing.
- Audio → UI display data (meters, spectra, LFO/envelope phase) uses `triple_buffer` on those
  same links where needed.

**Modules on the audio side** are stored as `ModuleHandle` (`enum_dispatch` over `SynthModule`)
in `FxHashMap<ModuleId, ModuleHandle>`. Output slots live in `OutputsArena`; per-voice routing
uses `VoiceRouter` / `ProcessContext` (`src/synth_engine/routing/`).

**`Output` is special.** It is always present at `OUTPUT_MODULE_ID` (`0`), created in
`SynthEngine::try_new`, and is not part of `ModuleConfig` / presets. User modules use ids ≥
`MIN_MODULE_ID` (`1`).

**Presets / persistence:** `EngineConfig` + `UiConfig` are `serde`-serializable
(`src/synth_engine/config.rs`, `src/synth_engine/ui_bridge/ui_config.rs`, `src/preset.rs`,
`src/presets.rs`). nih-plug persists them via `PresetWrapper` in `src/params.rs`.
`default_scheme.rs` builds the default patch.

**Engine params** (`EngineParams` in `config.rs`): polyphony, legato, block size, oversampling,
stereo vs mono spectrum, voice kill time, output gain, bandwidth. UI can change several of these
at runtime via the engine-level `UiEnd` / `AudioEnd` link.

## Routing

A patch is a directed graph. Edges target `InputId = (module_id, Input)`. Process order is a
topo sort of source → destination (including modulators).

| | |
| --- | --- |
| Data types | Audio, Control, Spectral |
| Link kinds | **Direct** (exactly one source) or **Mixed** (many sources + optional modulator) |
| Amount | Implicit `1.0` for Direct; `StereoSample` per Mixed source |
| Spectral inputs | Direct only (`InputMeta.is_direct`) |

Full rules (validation, acyclicity, mutation APIs): `docs/module-routing-rules.md`.

`Input` is an untyped routing key shared across all modules; reuse existing variants where the
semantic matches (e.g. `Gain`, `Level`, `Cutoff`) and document units (dB vs. linear) as done in
`routing.rs`.

## The module pattern (important)

Every DSP module follows the **same four-part structure**. Using `amplifier` as the template:

- `modules/<name>.rs` — the module struct implementing `SynthModule`
  (`id`, `inputs`, `output_type`, slot wiring, `process`, `process_ui_events`, …).
  Real-time DSP lives here. Holds an `AudioEnd` to receive UI events and an `Option<UiEnd>`
  that gets `take()`n by the UI bridge.
- `modules/<name>/config.rs` — `<Name>Config`, a `serde` struct that fully describes the
  module's state. `from_config` / `get_config` round-trip through it for presets.
- `modules/<name>/link.rs` — the lock-free `UiEnd` / `AudioEnd` pair and `UiEvent` enum
  (`rtrb`; plus `triple_buffer` when the UI needs live meters/spectra/phase).
- `modules/<name>/ui_bridge.rs` — `<Name>UiBridge` (implements `ModuleUiBridge`): UI-thread
  handle that owns the `UiEnd`, mirrors `config`, and exposes setters that push events.

Editor surfaces:

- Detail panel: `src/editor/modules_ui/<name>_ui.rs` (wired via `ModuleType::ui` in `editor.rs`).
- Grid tile (most modules): `src/editor/grid/grid_widget/<name>_widget.rs` (wired in
  `grid_widget.rs`).

`Output` is an exception: audio-only module in `modules/output.rs`, no config/link/ui_bridge
subdir, no `ModuleConfig` variant; UI is `output_ui.rs` / `output_widget.rs`.

### Adding or modifying a module — checklist

1. Create `modules/<name>.rs` + `config.rs`, `link.rs`, `ui_bridge.rs` (copy `amplifier`).
2. Register the module in `synth_engine/modules.rs` (`pub mod` + re-exports).
3. Add `ModuleType::<Name>` and `ModuleHandle::<Name>` in `module_handle.rs`.
4. Add `ModuleConfig::<Name>` in `config.rs` and wire it in `SynthEngine::try_new` /
   `get_config` (`src/synth_engine.rs`). Add `add_<name>` via `add_module_method!`.
5. Add `ModuleBridge::<Name>` and a match arm in `UiBridge::insert_module_bridge`.
6. Add the editor detail panel and `ModuleType::ui` arm; add a grid widget if the module
   appears on the patch grid.

Use the param macros in `synth_module.rs` (`set_smoothed_param!`, `get_smoothed_param!`,
`set_stereo_param!`, etc.) for the standard stereo/smoothed parameter plumbing.

## Conventions & constraints

- **Everything is stereo.** Use `StereoSample` (`synth_engine/stereo_sample.rs`); each channel
  is independent and `NUM_CHANNELS == 2`. Audio is `f32` (`Sample`).
- **No allocation on the audio thread.** `process` is wrapped in
  `assert_no_alloc::assert_no_alloc(...)` in `src/lib.rs`. Do not allocate, lock contended
  mutexes, or block inside `SynthModule::process` or anything it calls. Pre-allocate scratch
  buffers in the module struct (see `Amplifier::buffers`).
- **Buffers are fixed-size.** Time-domain `Buffer` is `[Sample; 257]` (`BUFFER_SIZE`);
  `SpectralBuffer` is `[ComplexSample; 1024]` (`SPECTRAL_BUFFER_SIZE`, `SPECTRUM_BITS == 10`).
  Spectral modules use `realfft`.
- **UI ↔ audio only via the link / ring-buffer / triple-buffer mechanism**, never by mutating
  engine state from the UI thread while audio runs. Preset/structural changes go through
  `EngineFactory` swaps.
- Voice limits: `MAX_VOICES == 20` (internal slots); user polyphony caps at
  `MAX_AVAILABLE_VOICES == 16` (`MAX_VOICES - 4`). Oscillator unison: up to 16
  (`MAX_UNISON_VOICES`).
- Prefer `parking_lot` locks, `rustc_hash::FxHashMap`, `smallvec`, and `enum_dispatch`
  consistent with existing code.

## Gotchas

- The engine is rebuilt from scratch on preset load; don't assume a module instance is stable
  across a preset change — the audio thread re-fetches it from the factory.
- Keep `config.rs` / module configs serde-compatible with existing presets; adding fields
  generally needs sensible `Default`s so old presets still deserialize.
- `Output` and engine-level params (block size, oversampling, …) are not ordinary module
  configs; touch the dedicated paths in `SynthEngine` / `UiBridge` instead of inventing a
  `ModuleConfig::Output`.
- Routing mutations must stay acyclic; `setup_routing` rejects cycles. Prefer the existing
  `UiBridge` / engine link helpers over hand-editing `input_sources`.

## Testing

- Tests live next to the code they cover (e.g. `src/synth_engine/voices_handler/tests.rs`,
  `src/synth_engine/tests.rs`, included via `#[cfg(test)] mod tests;`). Run them with
  `cargo test`.
- Performance benchmarks use [Criterion](https://github.com/bheisler/criterion.rs) in
  `benches/synth_engine.rs`. Coverage reports use
  [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov). See `TOOLS.md` for
  commands and workflows.

## Behavioral Guidelines

### Think Before Coding

Do not assume or hide confusion. Surface assumptions and tradeoffs before implementing.

- State assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them instead of choosing silently.
- If something is unclear, stop, name what is confusing, and ask.

### Simplicity First

Write the minimum code that solves the requested problem.

- Do not add features beyond what was asked.
- Do not add abstractions for single-use code.
- Do not add flexibility or configurability that was not requested.
- Do not add error handling for impossible scenarios.
- If a change is becoming much larger than necessary, simplify before continuing.
