# Module routing rules

A patch is a directed graph. Edges target `InputId = (module_id, Input)`. Process order is a topo sort of source → destination (including modulators).

|                  |                          |
| ---------------- | ------------------------ |
| Data types       | Audio, Control, Spectral |
| Link kinds       | Direct, Mixed            |
| Output module id | `0`                      |
| User module ids  | `≥ 1`                    |

## Link kinds

| Kind   | Cardinality on dst                  | Amount                    | Modulation         |
| ------ | ----------------------------------- | ------------------------- | ------------------ |
| Direct | Exactly one source (replaces prior) | Implicit `1.0`            | Forbidden          |
| Mixed  | Many sources; each `src` once       | `StereoSample` per source | Optional modulator |

Input kind is fixed by `InputMeta.is_direct`, declared by the module in `inputs()` via the `InputMeta` constructors (`routing.rs`): `direct_audio` / `audio`, `direct_control` / `control`, `spectral`. Direct and Mixed cannot share an `InputId`. Spectral inputs are Direct only (`InputMeta::spectral` always sets `is_direct`).

## Modulators

A modulator is an optional third module on a **Mixed** edge `src → dst`. It does not replace `src`; it scales that source’s contribution into `dst`:

`contribution = src × amount × modulator`

Rules:

- Direct links cannot be modulated.
- The edge `src → dst` must already exist; otherwise `set_link_modulation` fails with `"Invalid node."`.
- The modulator must pass `can_be_linked(modulator, dst, mixed)` (same type/kind rules as a Mixed source to that input).
- Modulation is per edge: each Mixed `src → dst` may have its own modulator.
- The destination depends on the modulator in the topo sort (modulator runs before `dst`).
- A module cannot be both a Mixed source and a modulator on the same `dst` at once — adding it as a source clears its modulator role on that input.

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

`setup_routing` builds the topo sort (`calc_execution_order`) over all links: each destination depends on each of its sources and modulators; unlinked modules are included too. Cycles → `"Cycles detected!"` and the routing update is rejected (the previous routing stays in place). In the resulting order `Output` (`OUTPUT_MODULE_ID`) is always moved to the end. `setup_routing` also reassigns input/output slots (`setup_slots`).

## Link mutations

All mutations except `update_link_amount` rebuild the full link list from `get_links()`, apply the change, and call `setup_routing`. They live on `SynthEngine` (`src/synth_engine.rs`); the UI calls them through the `UiBridge` wrappers of the same name (note `UiBridge::add_link` → `SynthEngine::add_mixed_link`), which lock the engine and refresh the cached `routing` state.

| Operation                                            | Effect                                                                                                                 |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `set_direct_link`                                    | `can_be_linked(src, dst, true)`; replace all sources on `dst` with one Direct                                          |
| `add_mixed_link`                                     | `can_be_linked(src, dst, false)`; append/replace Mixed `src→dst`; clear `src` if it was a modulator on `dst`           |
| `set_link_modulation`                                | Edge must exist and be Mixed; modulator must pass `can_be_linked(modulator, dst, false)`                               |
| `remove_link_modulation`                             | Clear the modulator on Mixed `src→dst`; no-op if none                                                                  |
| `update_link_amount`                                 | Mixed amounts only; does **not** rebuild routing. Sent lock-free from the UI via the engine `UiEnd` (`UiBridge::set_link_amount` → `UiEvent::LinkAmount`) and applied on the audio thread |
| `remove_link` / `remove_input_links` / `remove_output_links` | Drop matching edges; `remove_output_links` also clears modulators pointing at `src`                            |
| `remove_module`                                      | Free the output slot; drop all edges touching the module; clear modulators pointing at it                             |
| `set_config_links` (preset load)                     | Per link: skip if `src`/modulator fails `can_be_linked` or `src+dst` already present; Direct replaces existing sources on `dst`; Mixed clears `src`'s modulator role on `dst` |
| `refresh_routing`                                    | Re-validate `get_links()` with `can_be_linked`; drop invalid links, clear invalid modulators; `setup_routing`          |

`refresh_routing` is triggered by `UiBridge::update_routing` whenever a `ModuleUiBridge::update()` returns `true` — i.e. when a module's `inputs()` may have changed at runtime (Mixer / Spectral Mixer changing input count or volume type). Links to inputs that no longer exist are dropped.
