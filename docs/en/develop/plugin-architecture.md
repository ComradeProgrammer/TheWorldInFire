# Rules Plugin Architecture

Game rules are pluggable. A small kernel owns the game's state and turn structure. Rules plugins compiled to WebAssembly decide what every phase and command does. The official NATO rules are themselves such a plugin, bundled with the kernel and always loaded.

## Status

Implemented:

- The kernel (`crates/ooaw-core`) with a Wasmtime host, and the shared plugin interface crate (`crates/ooaw-plugin-api`), which also contains the Rust plugin SDK.
- The official NATO rules (`crates/plugins/nato-official`, package `ooaw-nato`). They are compiled to WebAssembly at build time, embedded in the kernel, and run as a WebAssembly plugin in the desktop app, the `ooaw-engine` process, and every test.
- An example third-party plugin (`crates/plugins/examples/night-fighting`) used to test guarded loading and manifest validation.

Not yet implemented: loading third-party plugins in the desktop app, saving or replaying games, and the other planned work at the end of this document.

## Responsibilities

```text
┌───────────── ooaw-core (kernel, native) ─────────────┐
│ authoritative state · seeded dice · turn sequencer   │
│ fixed commands · all-or-nothing execution · snapshots │
│ plugin registry · Wasmtime sandbox                    │
└───────────▲──────────────────────────────┬───────────┘
            │ imports: roll, filter, log   │ exports: one per RulesPlugin method
┌───────────┴──────────────────────────────▼───────────┐
│ one rules plugin (WebAssembly)                       │
│ ooaw-nato (official, bundled) or a third-party plugin │
└──────────────────────────────────────────────────────┘
```

The kernel knows nothing about NATO rules. It owns:

- **State.** The current turn position, units, city control, the pending player decision, and rules-owned state entries.
- **Dice.** One SplitMix64 generator seeded from the game ID. Plugins roll it through an import, so every outcome is reproducible from the seed and the command history.
- **Turn sequence.** It walks the scenario's phase steps, emits `phaseStarted`, `phaseEnded`, `gameTurnStarted`, and `gameCompleted`, and runs automatic phases.
- **Commands.** `GameCommand` is a closed enum shared by the kernel and every plugin. `endPhase` is handled by the kernel; every other command goes to the game's sole plugin, which decides how to validate and execute it. Queries are still named in the manifest.
- **Transactions.** Before each command the kernel copies its state and dice. If the command is rejected, or a plugin faults, both are restored and the revision is unchanged.
- **Consistency checks.** Plugins change state only through checked changes: units must be on known hexes with valid step indexes, cities must exist, and rules entries may not reuse kernel field names. The kernel checks no game rule.

A rules plugin owns everything else: scenarios and their content, phase behaviour, command validation and execution, previews, and its own state, which it stores in the kernel.

## Crates

| Crate | Role |
| --- | --- |
| `ooaw-plugin-api` | The shared model (commands, identifiers, map, phases, sides, generic units, game state) and the plugin contract. `sdk` holds the `RulesPlugin` trait, the `export_plugin!` macro, host functions (`sdk::host::roll`, `filter`, `log`), and `sdk::mirror` helpers that compute changes; `protocol` holds the types in the trait's signatures that the kernel also uses (manifest, attachment, scenario setup, changes). `abi` declares the `extern "C"` functions between the kernel and a plugin. Builds natively and for `wasm32`. |
| `ooaw-core` | The kernel: `GameEngine`, the Wasmtime runtime, the bundled official plugin, and the `ooaw-engine` binary. |
| `ooaw-nato` | The official NATO rules. As `cdylib` it is the WebAssembly plugin; as `rlib` it is a typed model for tests and for plugins that extend these rules. |
| `ooaw-example-night-fighting` | Example third-party plugin, built with the kernel for its tests. |

All crates, together with `src-tauri`, form one Cargo workspace rooted at the repository.

## Runtime

Plugins run in Wasmtime, compiled once per process with Cranelift, with no WASI imports. The only way a plugin touches the outside world is the kernel's `ooaw` imports.

- **Determinism.** NaN canonicalization is on and relaxed SIMD and threads are off, so a plugin computes the same results on every machine. Randomness exists only through `roll`.
- **Sandboxes.** A game that loads only bundled plugins runs in the *trusted* sandbox, compiled without interruption checks. A game with any third-party plugin runs in the *guarded* sandbox. There, each top-level call has a wall-clock deadline of about ten seconds, enforced by epoch interruption, which stops a plugin that never returns. The deadline never changes the result of a call that finishes. Fuel metering was measured and rejected: it made rules code about 40% slower.
- **Memory.** Each instance may use up to 512 MiB of linear memory.
- **Faults.** A trap, deadline, or malformed response makes the call fail with `pluginFault`. The kernel rolls the command back and replaces the instance, attaching the new one to the game, before its next call.
- **Start-up.** The bundled plugin is compiled on first use, in about 0.2 s for a release build. The desktop app compiles it on a background thread at launch.

Each game has one plugin instance and one Wasmtime store. The store's data holds the authoritative kernel state and plugin bookkeeping.

## Plugin contract (ABI version 4)

Plugins are written in Rust. The contract between the kernel and a plugin is the `RulesPlugin` trait in `ooaw-plugin-api`, together with the types in its signatures. A plugin implements the trait and invokes `export_plugin!` once. No other language is supported.

The binary interface is a set of `extern "C"` functions declared in `crates/ooaw-plugin-api/src/abi.rs`, each with a name constant and a function-pointer signature. The plugin's exports mirror the trait's methods one for one; `export_plugin!` generates them from the plugin's implementation and checks each against its signature at compile time. The plugin's imports are the kernel's services, wrapped by `sdk::host` and checked the same way. The kernel looks every function up by its name.

WebAssembly functions take and return only numbers, so other values live in the plugin's linear memory and pass as a pointer and a byte length. Text (a filter name, a log message) is UTF-8. Every other value is MessagePack, written by `abi::encode` and read by `abi::decode` on both sides; several arguments travel as one tuple. A function that returns a value returns `(ptr << 32) | len`, and the kernel frees the buffer with `ooaw_free`. A fallible method returns a `Result<T, RuleError>`, so a rule rejection crosses the boundary as data rather than as a trap. `GameCommand` is a shared, internally tagged enum; events, markers, and rules state remain open-ended values.

| Export | Trait method | Argument → result |
| --- | --- | --- |
| `memory` | — | Linear memory. |
| `ooaw_abi_version() -> i32` | — | Returns `ABI_VERSION`, now 4. |
| `ooaw_alloc(len) -> ptr` | — | Buffer for the kernel to write an argument into. |
| `ooaw_free(ptr, len)` | — | Frees a result buffer. |
| `ooaw_manifest() -> i64` | `manifest` | → `PluginManifest` |
| `ooaw_set_turn(game_turn, step_index, completed)` | `set_turn` | Plain numbers; no result. |
| `ooaw_sync(ptr, len) -> i64` | `sync` | `GameState` → `Result<()>` |
| `ooaw_take_changes() -> i64` | `take_changes` | → `Result<Vec<Change>>` |
| `ooaw_create_game(ptr, len) -> i64` | `create_game` | `(GameId, ScenarioRequest)` → `Result<ScenarioSetup>` |
| `ooaw_attach(ptr, len) -> i64` | `attach` | `Attachment` → `Result<()>` |
| `ooaw_apply_setup(ptr, len) -> i64` | `apply_setup` | setup → `Result<Vec<event>>` |
| `ooaw_phase_started(ptr, len) -> i64` | `phase_started` | `PhaseDefinition` → `Result<Vec<event>>` |
| `ooaw_phase_ending(ptr, len) -> i64` | `phase_ending` | `PhaseDefinition` → `Result<Vec<event>>` |
| `ooaw_command(ptr, len) -> i64` | `command` | `GameCommand` → `Result<Vec<event>>` |
| `ooaw_query(ptr, len) -> i64` | `query` | `(name, input)` → `Result<value>` |
| `ooaw_filter(ptr, len) -> i64` | `filter` | `(name, input, value)` → `Result<value>` |

| Import (module `ooaw`) | SDK function | Purpose |
| --- | --- | --- |
| `roll(sides) -> i32` | `host::roll` | Next value, `1..=sides`, of the game's dice. Traps during queries and filters. |
| `filter_active(ptr, len) -> i32` | `host::filter_active` | Always 0 in the single-plugin kernel; retained for ABI compatibility. |
| `filter(ptr, len) -> i32` | `host::filter` | Compatibility import that applies pending changes and returns the value unchanged. |
| `host_read(ptr)` | — | Copies the result of the last `filter` into plugin memory. |
| `log(ptr, len)` | `host::log` | Writes a message to the kernel's log. |

One kernel call to a plugin method is a short sequence of these calls: `ooaw_set_turn`; `ooaw_sync` if the plugin's mirror is stale; the method; and, after a successful method that may change state, `ooaw_take_changes`. `create_game` and `attach` start the mirror afresh, so they need no sync and report no changes. A stateless plugin is never synced or asked for changes.

`ABI_VERSION` changes whenever these functions or how they pass values change. The kernel rejects a plugin built against another version, so a plugin must be built with a matching `ooaw-plugin-api`. Version 1 passed JSON envelopes through a single `ooaw_dispatch` export; version 2 passed the same envelopes in MessagePack; version 4 replaces plugin-declared command strings with the shared `GameCommand` enum.

## Calls and routing

A plugin describes itself in a `PluginManifest`: identifier, version, ABI version, the phase types and queries it handles, its scenarios, and whether it is `stateless`. A game requires exactly one plugin, which must provide the selected scenario. Commands are not registered in the manifest: the kernel accepts only variants of the shared `GameCommand` enum and sends every non-kernel command to that one plugin.

| Method | Purpose | May change state |
| --- | --- | --- |
| `manifest` | Describe the plugin. | no |
| `create_game` | Build a scenario: summary, map, turn sequence, opening state. | no |
| `attach` | Reattach a replacement instance after a fault. | no |
| `apply_setup` | Lay out a custom starting situation in the plugin's format. | yes |
| `phase_started`, `phase_ending` | Phase work; rejecting `phase_ending` keeps the phase current. | yes |
| `command` | Execute a player command. | yes |
| `query` | Answer a read-only preview. | no |
| `filter` | Adjust a value at an extension point. | no |

Read-only calls that report changes or roll dice fail with `readOnlyViolation` or a trap.

## State mirroring and changes

The plugin keeps a mirror of the mutable state it uses, so the kernel does not resend the whole state with every call. The kernel tracks a generation number and sends the full `GameState` (units, city control, pending decision, rules entries) through `sync` only when the mirror is stale: on the first call, after a rollback, or after a direct edit. It sends the turn position through `set_turn` before every call.

A plugin changes its mirror directly, then reports the difference as `Change`s:

| Change | Effect |
| --- | --- |
| `putUnit` | Add a unit or replace it whole. |
| `updateUnit` | Replace named top-level fields; unmentioned fields are kept. |
| `removeUnit` | Remove a unit from play. |
| `setCityControl` | Change a city's controller. |
| `setRules` | Replace one rules-state entry. |
| `setPendingDecision` | Set or clear the decision players must make. |

The kernel's unit is generic: identity, side, nation, type, formation, traits, a step track whose printed values the rules define, a strength-step index, a location, and *markers*, the rules-owned fields flattened into the unit object. The NATO rules' typed unit serializes to exactly this shape, with `supply`, `trainStatus`, and `disruption` as markers. Rules-owned state entries are flattened into the snapshot in the same way. The client-facing JSON therefore kept the shape it had before the split.

The NATO plugin keeps a baseline of what it last reported. After each mutating call, the SDK's `mirror` helpers compare the baseline with the current mirror field by field.

## Filters

The filter functions and manifest field remain in ABI version 4, but the kernel permits exactly one plugin per game. Consequently `filter_active` always returns false and filter calls return their value unchanged. Cross-plugin rule extension is not active.

## Writing a plugin in Rust

An outline, with bodies elided; `crates/plugins/examples/night-fighting/src/lib.rs` is a complete plugin.

```rust
use ooaw_plugin_api::export_plugin;
use ooaw_plugin_api::sdk::{Attachment, RulesPlugin};

#[derive(Default)]
struct MyRules { /* mirror and baseline */ }

impl RulesPlugin for MyRules {
    fn manifest(&self) -> PluginManifest { /* phases, queries, filters, scenarios */ }
    fn attach(&mut self, attachment: Attachment) -> Result<(), RuleError> { Ok(()) }
    fn set_turn(&mut self, turn: TurnPosition) {}
    fn sync(&mut self, state: Value) -> Result<(), RuleError> { /* load mirror */ }
    fn take_changes(&self) -> Result<Vec<Change>, RuleError> { /* mirror helpers */ }
    fn filter(&self, name: &str, input: Value, value: Value) -> Result<Value, RuleError> { Ok(value) }
}

export_plugin!(MyRules);
```

Build it with `crate-type = ["cdylib", "rlib"]` for `wasm32-unknown-unknown`, then load the bytes with `PluginModule::from_wasm` and pass it as the game's sole plugin.

## Build

`crates/ooaw-core/build.rs` runs Cargo to build `ooaw-nato` and the example plugin in release mode for `wasm32-unknown-unknown`, in the separate target directory `target/wasm-plugins`. It then copies the modules into `OUT_DIR` for the kernel to embed. Every build of the kernel, including `npm run tauri dev` and `cargo check`, therefore needs the target:

```sh
rustup target add wasm32-unknown-unknown
```

The official module is about 2 MB, including the embedded map and campaign data. Cranelift and the Wasmtime compiler packages are optimised even in dev builds (workspace `Cargo.toml`), because unoptimised Cranelift compiles plugins many times more slowly.

## Testing

- `crates/plugins/nato-official/tests/rules.rs` and `campaign_scenarios.rs`: the NATO rules suite, run through the kernel and the WebAssembly plugin. The harness in `tests/common/mod.rs` reads state through the typed NATO model and edits it with `GameEngine::edit_state`. The kernel-gated `debug.checkSupply` command runs a supply check on demand.
- `crates/ooaw-core/tests/kernel.rs`: routing, transactions, read-only queries, debug-command gating, and direct edits.
- `crates/ooaw-core/tests/plugins.rs`: guarded third-party loading and the single-plugin constraint.
- `crates/ooaw-core/tests/engine_black_box.rs` and `campaign_scenarios.rs`: the `ooaw-engine` JSON process.
- Unit tests for change diffing (`ooaw-plugin-api::sdk::mirror`) and change checks (`ooaw-core`).

## Performance

The following release-build timings per call were measured on the development machine against the pre-split native engine, with the JSON messages of ABI version 1. They have not been re-measured with MessagePack.

| Operation | Native engine | Plugin, trusted sandbox | Plugin, guarded sandbox |
| --- | --- | --- | --- |
| BALTAP `movementModes` (one unit, all systems) | 4.4 ms | 4.4 ms | 6.6 ms |
| 341-unit campaign `movementModes` | 10.0 ms | 9.9 ms | 14.5 ms |
| BALTAP `endPhase` | 0.9 ms | 1.0 ms | 1.3 ms |
| 341-unit campaign `endPhase` | 9.2 ms | 11.7 ms | 15.5 ms |

Rules code runs at native speed in the trusted sandbox. The remaining cost of `endPhase` in large scenarios comes from reporting and checking changes.

## Planned work

- Load a third-party replacement plugin in the desktop app (discovery, selection, and manifest display). Today only `GameEngine::with_plugins` accepts one.
- Saves and replays must record the plugin version and module hash, and the dice state.
- Supporting multiple plugins again would require restoring cross-plugin filter routing and nested-call tracking.
- The sole plugin handles every non-kernel command; phases and queries still use named handlers, and there are no observer hooks.
- The map model's terrain, city, and hexside kinds are closed enums shared by all plugins.
- Precompiled modules (`.cwasm`) would remove the start-up compilation.
- The kernel cannot yet show a plugin's own UI. The frontend currently understands only the official rules' state and previews.
- The fault-recovery path (instance replacement after a trap) has no automated test.
