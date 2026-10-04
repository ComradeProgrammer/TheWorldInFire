# Rules Plugin Architecture

Game rules are pluggable. A small kernel owns the game's state and turn structure. Rules plugins compiled to WebAssembly decide what every phase and command does. The official NATO rules are themselves such a plugin, bundled with the kernel and always loaded.

## Status

Implemented:

- The kernel (`crates/ooaw-core`) with a Wasmtime host, the shared protocol crate (`crates/ooaw-plugin-api`), and the Rust plugin SDK (`crates/ooaw-plugin-sdk`).
- The official NATO rules (`plugins/nato-official`, package `ooaw-nato`). They are compiled to WebAssembly at build time, embedded in the kernel, and run as a WebAssembly plugin in the desktop app, the `ooaw-engine` process, and every test.
- Filters (named extension points) in the official combat and air-strike rules.
- An example third-party plugin (`plugins/examples/night-fighting`) that the kernel's tests load alongside the official rules.

Not yet implemented: loading third-party plugins in the desktop app, saving or replaying games, and the other planned work at the end of this document.

## Responsibilities

```text
┌───────────── ooaw-core (kernel, native) ─────────────┐
│ authoritative state · seeded dice · turn sequencer   │
│ command routing · all-or-nothing commands · snapshots │
│ plugin registry · Wasmtime sandbox                    │
└───────────▲──────────────────────────────┬───────────┘
            │ imports: roll, host_call     │ exports: ooaw_dispatch
┌───────────┴──────────────────────────────▼───────────┐
│ rules plugins (WebAssembly)                          │
│ ooaw-nato (official, bundled) · third-party plugins   │
└──────────────────────────────────────────────────────┘
```

The kernel knows nothing about NATO rules. It owns:

- **State.** The current turn position, units, city control, the pending player decision, and rules-owned state entries.
- **Dice.** One SplitMix64 generator seeded from the game ID. Plugins roll it through an import, so every outcome is reproducible from the seed and the command history.
- **Turn sequence.** It walks the scenario's phase steps, emits `phaseStarted`, `phaseEnded`, `gameTurnStarted`, and `gameCompleted`, and runs automatic phases.
- **Routing.** `endPhase` is handled by the kernel. Every other command and every query goes to the plugin that declared it.
- **Transactions.** Before each command the kernel copies its state and dice. If the command is rejected, or a plugin faults, both are restored and the revision is unchanged.
- **Consistency checks.** Plugins change state only through checked changes: units must be on known hexes with valid step indexes, cities must exist, and rules entries may not reuse kernel field names. The kernel checks no game rule.

A rules plugin owns everything else: scenarios and their content, phase behaviour, command validation and execution, previews, and its own state, which it stores in the kernel.

## Crates

| Crate | Role |
| --- | --- |
| `ooaw-plugin-api` | Shared vocabulary: identifiers, map, phases, sides, generic units, and the `protocol` module (ABI, calls, changes, manifest). Builds natively and for `wasm32`. |
| `ooaw-plugin-sdk` | Writing plugins in Rust: the `RulesPlugin` trait, the `export_plugin!` macro, host functions (`host::roll`, `host::filter`, `host::log`), and `mirror` helpers that compute changes. |
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

All plugin instances of one game share one Wasmtime store. The store's data holds the kernel state, so while one plugin is running, the kernel can apply its changes and call the other plugins that take part in a filter.

## Binary interface (ABI version 1)

Every message is UTF-8 JSON. A plugin is a core WebAssembly module that exports:

| Export | Purpose |
| --- | --- |
| `memory` | Linear memory. |
| `ooaw_abi_version() -> i32` | Returns 1. |
| `ooaw_alloc(len) -> ptr` | Buffer for the kernel to write a request into. |
| `ooaw_free(ptr, len)` | Frees a request buffer or a response. |
| `ooaw_dispatch(ptr, len) -> i64` | Handles one `DispatchRequest`; returns `(response_ptr << 32) \| response_len` for a `DispatchResponse`. |

It may import from module `ooaw`:

| Import | Purpose |
| --- | --- |
| `roll(sides) -> i32` | Next value, `1..=sides`, of the game's dice. Traps during queries and filters. |
| `host_call(ptr, len) -> i32` | Sends a `HostRequest` (`filter` or `log`); returns the response length. |
| `host_read(ptr)` | Copies that response into guest memory. |

The exact message types are in `crates/ooaw-plugin-api/src/protocol.rs`. JSON keeps the protocol easy to follow from any language while the game model is still changing. A typed WIT interface can replace it once the model settles.

## Calls and routing

A plugin describes itself in a `PluginManifest`: identifier, version, ABI version, the phase types, command types, queries, and filters it handles, its scenarios, and whether it is `stateless`. A game is created with an ordered list of plugins (the official one first):

- the scenario comes from the last plugin listing its ID;
- a phase, command, or query is handled by the last plugin declaring it;
- every plugin listing a filter takes part in it, in load order.

| Call | Purpose | May change state |
| --- | --- | --- |
| `manifest` | Describe the plugin. | no |
| `createGame` | Build a scenario: summary, map, turn sequence, opening state. | no |
| `attach` | Serve an existing game (other plugins, or a replaced instance). | no |
| `applySetup` | Lay out a custom starting situation in the plugin's format. | yes |
| `phaseStarted`, `phaseEnding` | Phase work; rejecting `phaseEnding` keeps the phase current. | yes |
| `command` | Execute a player command. | yes |
| `query` | Answer a read-only preview. | no |
| `filter` | Adjust a value at an extension point. | no |

Read-only calls that report changes or roll dice fail with `readOnlyViolation` or a trap.

## State mirroring and changes

Each plugin keeps a mirror of the mutable state it uses, so the kernel does not resend the whole state with every call. The kernel tracks a generation number and the generation each plugin's mirror matches. A request carries the full `GameState` (units, city control, pending decision, rules entries) only when the mirror is stale: on the first call, after a rollback, after a direct edit, or after another plugin changed state. The turn position travels with every call.

A plugin changes its mirror directly, then reports the difference as `Change`s:

| Change | Effect |
| --- | --- |
| `putUnit` | Add a unit or replace it whole. |
| `updateUnit` | Replace named top-level fields; markers other plugins own are kept. |
| `removeUnit` | Remove a unit from play. |
| `setCityControl` | Change a city's controller. |
| `setRules` | Replace one rules-state entry. |
| `setPendingDecision` | Set or clear the decision players must make. |

The kernel's unit is generic: identity, side, nation, type, formation, traits, a step track whose printed values the rules define, a strength-step index, a location, and *markers*, the rules-owned fields flattened into the unit object. The NATO rules' typed unit serializes to exactly this shape, with `supply`, `trainStatus`, and `disruption` as markers. Rules-owned state entries are flattened into the snapshot in the same way. The client-facing JSON therefore kept the shape it had before the split.

The NATO plugin keeps a baseline of what it last reported. After each mutating call, the SDK's `mirror` helpers compare the baseline with the current mirror field by field.

## Filters

A filter is a named extension point. The plugin that needs a value computes it, then passes it through every other plugin taking part in the filter, in load order. Each plugin may return the value adjusted. If no other plugin takes part, the call is skipped. Before calling out, the caller sends its pending changes, so the other plugins see current state. Filters are read-only.

The official rules expose (`plugins/nato-official/src/filters.rs`):

| Filter | Input | Value |
| --- | --- | --- |
| `nato.combat.attackStrength` | `{ sideId, objective, unitId }` | one attacker's `UnitStrength` after the printed modifiers |
| `nato.combat.defenseStrength` | `{ sideId, objective, unitId }` | one defender's `UnitStrength` after the printed modifiers |
| `nato.combat.columnShifts` | `{ sideId, objective, attackingUnitIds, offensiveSupport }` | the battle's column shifts before the two-column limit |
| `nato.combat.result` | `{ sideId, objective, finalColumn, dieRoll }` | the `CombatResult` read from the CRT |
| `nato.air.strikeModifier` | `{ sideId, hexId, unitIds }` | one target's Strike Table die-roll modifier |

Battle previews use the same filters as resolution, so a preview always matches the battle. A strength modifier or column shift added by another plugin carries the plugin's own name, such as `example.nightFighting`. The battle planner shows such a name as written.

The example plugin `plugins/examples/night-fighting` appends a −1 shift to every battle. It takes part in `nato.combat.columnShifts` only, declares itself `stateless`, and uses plain JSON rather than the `ooaw-nato` types, as a plugin in another language would. `crates/ooaw-core/tests/plugins.rs` checks that it shifts both the preview and the resolved battle.

## Writing a plugin in Rust

An outline, with bodies elided; `plugins/examples/night-fighting/src/lib.rs` is a complete plugin.

```rust
use ooaw_plugin_sdk::{export_plugin, Attachment, RulesPlugin};

#[derive(Default)]
struct MyRules { /* mirror and baseline */ }

impl RulesPlugin for MyRules {
    fn manifest(&self) -> PluginManifest { /* phases, commands, queries, filters */ }
    fn attach(&mut self, attachment: Attachment) -> Result<(), RuleError> { Ok(()) }
    fn set_turn(&mut self, turn: TurnPosition) {}
    fn sync(&mut self, state: Value) -> Result<(), RuleError> { /* load mirror */ }
    fn take_changes(&self) -> Result<Vec<Change>, RuleError> { /* mirror helpers */ }
    fn filter(&self, name: &str, input: Value, value: Value) -> Result<Value, RuleError> { Ok(value) }
}

export_plugin!(MyRules);
```

Build it with `crate-type = ["cdylib", "rlib"]` for `wasm32-unknown-unknown`, then load the bytes with `PluginModule::from_wasm` and pass the module to `GameEngine::with_plugins` after the official plugin.

## Build

`crates/ooaw-core/build.rs` runs Cargo to build `ooaw-nato` and the example plugin in release mode for `wasm32-unknown-unknown`, in the separate target directory `target/wasm-plugins`. It then copies the modules into `OUT_DIR` for the kernel to embed. Every build of the kernel, including `npm run tauri dev` and `cargo check`, therefore needs the target:

```sh
rustup target add wasm32-unknown-unknown
```

The official module is about 2 MB, including the embedded map and campaign data. Cranelift and the Wasmtime compiler packages are optimised even in dev builds (workspace `Cargo.toml`), because unoptimised Cranelift compiles plugins many times more slowly.

## Testing

- `plugins/nato-official/tests/rules.rs` and `campaign_scenarios.rs`: the NATO rules suite, run through the kernel and the WebAssembly plugin. The harness in `tests/common/mod.rs` reads state through the typed NATO model and edits it with `GameEngine::edit_state`. The kernel-gated `debug.checkSupply` command runs a supply check on demand.
- `crates/ooaw-core/tests/kernel.rs`: routing, transactions, read-only queries, debug-command gating, and direct edits.
- `crates/ooaw-core/tests/plugins.rs`: a third-party filter plugin in the guarded sandbox.
- `crates/ooaw-core/tests/engine_black_box.rs` and `campaign_scenarios.rs`: the `ooaw-engine` JSON process.
- Unit tests for change diffing (`ooaw-plugin-sdk`) and change checks (`ooaw-core`).

## Performance

The following release-build timings per call were measured on the development machine against the pre-split native engine.

| Operation | Native engine | Plugin, trusted sandbox | Plugin, guarded sandbox |
| --- | --- | --- | --- |
| BALTAP `movementModes` (one unit, all systems) | 4.4 ms | 4.4 ms | 6.6 ms |
| 341-unit campaign `movementModes` | 10.0 ms | 9.9 ms | 14.5 ms |
| BALTAP `endPhase` | 0.9 ms | 1.0 ms | 1.3 ms |
| 341-unit campaign `endPhase` | 9.2 ms | 11.7 ms | 15.5 ms |

Rules code runs at native speed in the trusted sandbox. The remaining cost of `endPhase` in large scenarios comes from reporting and checking changes.

## Planned work

- Load third-party plugins in the desktop app (discovery, selection per game, and manifest display). Today only `GameEngine::with_plugins` accepts them.
- Saves and replays must record the plugin list with versions and module hashes, and the dice state.
- A plugin cannot call into a plugin that is already handling a call (`reentrantPluginCall`). Filter participants cannot yet ask other plugins for queries.
- Each phase, command, and query has a single owner; there are no observer hooks yet.
- The map model's terrain, city, and hexside kinds are closed enums shared by all plugins.
- Stale plugins receive the whole state; sending only the changes since their last sync would make games with several stateful plugins cheaper.
- Precompiled modules (`.cwasm`) would remove the start-up compilation.
- The kernel cannot yet show a plugin's own UI. The frontend understands the official rules' state and previews and shows other plugins' names as written.
- The fault-recovery path (instance replacement after a trap) has no automated test.
