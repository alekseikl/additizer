# Module routing rules

A patch is a directed graph. Edges target `InputId = (module_id, Input)`. Process order is a topo sort of source → destination.

|                  |                          |
| ---------------- | ------------------------ |
| Data types       | Audio, Control, Spectral |
| Link kinds       | Direct, Mixed            |
| Output module id | `0`                      |
| Fixed pitch id   | `-1` (`PITCH_MODULE_ID`) |
| User module ids  | `≥ 1`                    |

## Link kinds

| Kind   | Cardinality on dst                  | Amount                    |
| ------ | ----------------------------------- | ------------------------- |
| Direct | Exactly one source (replaces prior) | Implicit `1.0`            |
| Mixed  | Many sources; each `src` once       | `StereoSample` per source |

Input kind is fixed by `InputMeta.is_direct`, declared by the module in `inputs()` via the `InputMeta` constructors (`routing.rs`): `direct_audio` / `audio`, `direct_control` / `control`, `spectral`. Direct and Mixed cannot share an `InputId`. Spectral inputs are Direct only (`InputMeta::spectral` always sets `is_direct`).

A mixed contribution is `src × amount`.

## Validation (`can_be_linked`)

Per-link checks only — does **not** check acyclicity. Checks run in this order; the first failure is returned as the error string.

| Rule             | Detail                                                                      | Error                                                                 |
| ---------------- | --------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Endpoints exist  | `src` and `dst.module_id` in `modules`                                      | `"Invalid node."`                                                     |
| Input exists     | `dst.input_type` in `dst_module.inputs()`                                   | `"Invalid destination input."`                                        |
| Kind matches     | Direct ↔ `is_direct`; Mixed ↔ `!is_direct`                                  | `"Mixed inputs require add_mixed_link."` / `"Direct inputs require set_direct_link."` |
| Types compatible | `data_types_compatible`: equal types, or Control → Audio                    | `"Data types mismatch."`                                              |

| src \\ dst | Audio | Control | Spectral |
| ---------- | ----- | ------- | -------- |
| Audio      | yes   | no      | no       |
| Control    | yes   | yes     | no       |
| Spectral   | no    | no      | yes      |

## Acyclicity (`setup_routing`)

`setup_routing` builds the topo sort (`process_order`) over all links: each destination depends on each of its sources; unlinked modules are included too. Cycles → `"Cycles detected!"` and the routing update is rejected (the previous routing stays in place). In the resulting order `Output` (`OUTPUT_MODULE_ID`) is always moved to the end. `setup_routing` also reassigns input/output slots (`setup_slots`).

Before sorting, `setup_routing` calls `prepare_links`. That drops hidden links, drops links that fail `can_be_linked`, and pre-wires the fixed pitch module (`PITCH_MODULE_ID`, `-1`) to every direct `Input::Pitch` that has no source. Each pre-wire is a direct link with `hidden: true`. Mixed links cannot be hidden. A pre-wire that would cycle is skipped. Adding a module, removing a pitch source, or changing `inputs()` rebuilds routing through `setup_routing`, so the pre-wire stays in place. An explicit pitch link replaces the hidden one; removing that link restores it.

Hidden links are omitted from presets (`get_config`) and from the UI routing snapshot (`RoutingState`), so automatic pitch cables are not drawn. The fixed pitch module is shown on the grid. A pitch cable drawn from it is an ordinary direct link (`hidden: false`) and is displayed. Pitch inputs with only a pre-wire still look unconnected.

## Link mutations

All mutations except `update_link_amount` rebuild the full link list from `get_links()`, apply the change, and call `setup_routing`. They live on `SynthEngine` (`crates/addi-engine/src/synth_engine.rs`); the UI calls them through the `UiBridge` wrappers of the same name (note `UiBridge::add_link` → `SynthEngine::add_mixed_link`), which lock the engine and refresh the cached `routing` state.

| Operation                                            | Effect                                                                                                                 |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `set_direct_link`                                    | `can_be_linked(src, dst, true)`; replace all sources on `dst` with one Direct                                          |
| `add_mixed_link`                                     | `can_be_linked(src, dst, false)`; append/replace Mixed `src→dst`                                                       |
| `update_link_amount`                                 | Mixed amounts only; does **not** rebuild routing. Sent lock-free from the UI via the engine `UiEnd` (`UiBridge::set_link_amount` → `UiEvent::LinkAmount`) and applied on the audio thread |
| `remove_link` / `remove_input_links` / `remove_output_links` | Drop matching edges                                                                                            |
| `remove_module`                                      | Free the output slot; drop all edges touching the module                                                               |
| `set_config_links` (preset load)                     | Per link: skip if `src` fails `can_be_linked` or `src+dst` already present; Direct replaces existing sources on `dst` |
| `refresh_routing`                                    | Rebuild from `get_links()` via `setup_routing` (`prepare_links` drops invalid links and restores pitch pre-wires) |

`refresh_routing` is triggered by `UiBridge::update_routing` whenever a `ModuleUiBridge::update()` returns `true` — i.e. when a module's `inputs()` may have changed at runtime (Mixer / Spectral Mixer changing input count or volume type). Links to inputs that no longer exist are dropped.
