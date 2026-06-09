# AGENTS.md

Guidance for AI agents working in this repository. Read this before making changes.

## Project overview

Additizer is a **modular synthesizer plugin** written in Rust. It builds as a
CLAP plugin (`cdylib`) and a standalone app, using the [`nih-plug`](https://github.com/robbert-vdh/nih-plug)
framework with an `egui`-based editor (`nih_plug_egui`, OpenGL via `baseview`).

The synth is a graph of **modules**. Some process audio in the time domain (oscillator,
mixer, amplifier, waveshaper), others operate on spectra (harmonic editor, spectral
filter/mixer/blend), and others are modulation sources (envelope, LFO, expressions/MPE,
external params). Modules are connected by **links** into a routing graph that is
topologically sorted and processed per voice/channel. See `README.md` for the user-facing
module list.

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
   `Additizer::process` (in `src/lib.rs`) splits the host buffer into blocks (≤ `MAX_BLOCK_SIZE`,
   `block_size()`), reorders note events, and drives the engine.
2. **UI thread** — owns `UiBridge` (`src/synth_engine/ui_bridge.rs`) and the `egui` editor
   (`src/editor.rs`). Reads/writes engine state without touching the audio thread directly.

**`EngineFactory`** (`src/engine_factory.rs`) is the shared bridge between them. It holds the
live `SynthEngine` and `UiConfig` inside `ArcSwap<Mutex<…>>`. Loading a preset swaps in a brand
new engine; the audio thread detects the swap via `engine_changed()` and picks it up at the next
`process` call.

**Communication is lock-free.** UI → audio parameter changes go through `rtrb` ring buffers
(the `UiEnd` / `AudioEnd` "link" pair), not by locking the engine during audio processing.

**Presets / persistence:** `EngineConfig` + `UiConfig` are `serde`-serializable
(`src/synth_engine/config.rs`, `src/preset.rs`, `src/presets.rs`). nih-plug persists them via
`PresetWrapper` in `src/params.rs`. `default_scheme.rs` builds the default patch.

## Directory layout

```
src/
  lib.rs                 # Plugin entry: nih-plug Plugin/ClapPlugin impl, process loop
  main.rs                # Standalone entry point
  params.rs              # nih-plug Params, preset persistence wrapper
  engine_factory.rs      # Shared engine/ui-config holder (ArcSwap)
  editor.rs              # egui editor shell + ModuleUi trait
  editor/                # UI widgets (sliders, inputs) and modules_ui/* per-module panels
  default_scheme.rs, preset.rs, presets.rs   # Default patch + preset (de)serialization
  synth_engine.rs        # SynthEngine: graph build, topo sort, per-block processing
  synth_engine/
    routing.rs           # ModuleType, Input, DataType, Router trait, ModuleLink
    synth_module.rs      # SynthModule + ModuleUiBridge traits, VoiceRouter, param macros
    config.rs            # EngineConfig / ModuleConfig / LinkConfig (serde)
    voices_handler.rs    # Voice allocation, note on/off/choke, legato
    buffer.rs            # Buffer / SpectralBuffer types and sizes
    ui_bridge.rs         # UiBridge: UI-side view of the engine
    modules/             # One module per file + its config/link/ui_bridge submodules
```

## The module pattern (important)

Every module follows the **same four-part structure**. Using `amplifier` as the template:

- `modules/<name>.rs` — the module struct implementing the `SynthModule` trait
  (`id`, `module_type`, `inputs`, `output`, `process`, `handle_ui_events`, output getters).
  Real-time DSP lives here. Holds an `AudioEnd` to receive UI events and an `Option<UiEnd>`
  that gets `take()`n by the UI bridge.
- `modules/<name>/config.rs` — `<Name>Config`, a `serde` struct that fully describes the
  module's state. `from_config` / `get_config` round-trip through it for presets.
- `modules/<name>/link.rs` — the `rtrb`-based `UiEnd`/`AudioEnd` event pair and `UiEvent` enum
  for lock-free UI → audio messaging. Usually just re-uses `create_link_pair()`.
- `modules/<name>/ui_bridge.rs` — `<Name>UiBridge` (implements `ModuleUiBridge`): UI-thread
  handle that owns the `UiEnd`, mirrors `config`, and exposes setters that push events.

The matching editor panel lives in `src/editor/modules_ui/<name>_ui.rs`.

### Adding or modifying a module — checklist

1. Create `modules/<name>.rs` + `config.rs`, `link.rs`, `ui_bridge.rs` (copy `amplifier`).
2. Register the module in `synth_engine/modules.rs` (`pub mod` + re-exports).
3. Add a `ModuleType::<Name>` variant in `routing.rs` and any new `Input` variants.
4. Add a `ModuleConfig::<Name>` variant in `config.rs` and wire it in `SynthEngine::try_new`
   (the `from_cfg!` match) and in `get_config` (`src/synth_engine.rs`).
5. Add the UI bridge to `UiBridge::insert_module_bridge` (`src/synth_engine/ui_bridge.rs`).
6. Add the editor panel in `src/editor/modules_ui/` and wire it into `editor.rs`.

Use the param macros in `synth_module.rs` (`set_smoothed_param!`, `get_smoothed_param!`,
`set_stereo_param!`, etc.) for the standard stereo/smoothed parameter plumbing.

## Conventions & constraints

- **Everything is stereo.** Use `StereoSample` (`synth_engine/stereo_sample.rs`); each channel
  is independent and `NUM_CHANNELS == 2`. Audio is `f32` (`Sample`).
- **No allocation on the audio thread.** `process` is wrapped in
  `assert_no_alloc::assert_no_alloc(...)` in `src/lib.rs`. Do not allocate, lock contended
  mutexes, or block inside `SynthModule::process` or anything it calls. Pre-allocate scratch
  buffers in the module struct (see `Amplifier::buffers`).
- **Buffers are fixed-size.** Time-domain `Buffer` is `[Sample; 257]`; `SpectralBuffer` is
  `[ComplexSample; 1024]` (1024 harmonics/bins). Spectral modules use `realfft`.
- **UI ↔ audio only via the link/ring-buffer mechanism**, never by mutating engine state from
  the UI thread while audio runs. Preset/structural changes go through `EngineFactory` swaps.
- Voice limits: `MAX_VOICES == 24`; the oscillator supports up to 16 unison voices.
- Prefer `parking_lot` locks, `rustc_hash::FxHashMap`, and `smallvec` consistent with existing code.

## Gotchas

- The engine is rebuilt from scratch on preset load; don't assume a module instance is stable
  across a preset change — the audio thread re-fetches it from the factory.
- `Input` is an untyped routing key shared across all modules; reuse existing variants where the
  semantic matches (e.g. `Gain`, `Level`, `Cutoff`) and document units (dB vs. linear) as done in
  `routing.rs`.
- Keep `config.rs` serde-compatible with existing presets; adding fields generally needs sensible
  `Default`s so old presets still deserialize.

## Testing

- Tests live next to the code they cover (e.g. `src/synth_engine/voices_handler/tests.rs`,
  included via `#[cfg(test)] mod tests;`). Run them with `cargo test`.

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
