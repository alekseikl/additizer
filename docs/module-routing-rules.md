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

Input kind is fixed by `InputMeta.is_direct`. Direct and Mixed cannot share an `InputId`. Spectral inputs are Direct only.

## Modulators

A modulator is an optional third module on a **Mixed** edge `src → dst`. It does not replace `src`; it scales that source’s contribution into `dst`:

`contribution = src × amount × modulator`

Rules:

- Direct links cannot be modulated.
- The modulator must pass `can_be_linked(modulator, dst, mixed)` (same type/kind rules as a Mixed source to that input).
- Modulation is per edge: each Mixed `src → dst` may have its own modulator.
- The destination depends on the modulator in the topo sort (modulator runs before `dst`).
- A module cannot be both a Mixed source and a modulator on the same `dst` at once — adding it as a source clears its modulator role on that input.

## Validation (`can_be_linked`)

Per-link checks only — does **not** check acyclicity.

| Rule             | Detail                                                                      |
| ---------------- | --------------------------------------------------------------------------- |
| Endpoints exist  | `src` and `dst.module_id` in `modules`                                      |
| Input exists     | `dst.input_type` in `dst_module.inputs()`                                   |
| Kind matches     | Direct ↔ `is_direct`; Mixed ↔ `!is_direct`                                  |
| Types compatible | Equal types, or Control → Audio. Spectral only to Spectral. Audio ↛ Control |

| src \\ dst | Audio | Control | Spectral |
| ---------- | ----- | ------- | -------- |
| Audio      | yes   | no      | no       |
| Control    | yes   | yes     | no       |
| Spectral   | no    | no      | yes      |

## Acyclicity (`setup_routing`)

`setup_routing` builds the topo sort over all links (destination depends on each source and modulator). Cycles → `"Cycles detected!"` and the routing update is rejected.

## Link mutations

| Operation                    | Effect                                                                     |
| ---------------------------- | -------------------------------------------------------------------------- |
| `set_direct_link`            | Replace all sources on `dst` with one Direct                               |
| `add_mixed_link`             | Append/replace Mixed `src→dst`; clear `src` if it was a modulator on `dst` |
| `set_link_modulation`        | Mixed only; modulator must pass `can_be_linked(..., false)`                |
| `update_link_amount`         | Mixed amounts only                                                         |
| `remove_*` / `remove_module` | Drop edges; clear modulators pointing at removed `src`                     |
| `set_config_links`           | Skip invalid / duplicate `src+dst`.                                        |
| `refresh_routing`            | Re-validate `get_links()` with `can_be_linked`; drop invalid links, clear invalid modulators; `setup_routing` |
