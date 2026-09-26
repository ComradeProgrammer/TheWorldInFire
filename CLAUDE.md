# OOAW Project Guide

## Project purpose

OOAW is a personal Rust-based digital wargame project. The first milestone is to reproduce the gameplay systems of the board game **NATO: The Cold War Goes Hot** closely enough to create a playable digital prototype. The reference game is a starting template for learning and validating the rules, map interaction, counter handling, turn sequence, combat, supply, air operations, and scenario setup. The project may develop its own presentation and mechanics later.

The intended application is a desktop 2D hex-and-counter wargame. It should support large maps, zooming and panning, counter selection and stacking, movement paths, phase-based turns, overlays such as supply and zones of control, deterministic combat resolution, saves, and replayable command history.

## Reference material

All externally collected rules, maps, counters, VASSAL modules, and other research material belong in the repository-root `internet/` directory.

Known reference material includes:

- `internet/NATO_Rules_2020.pdf`: the main NATO rules booklet.
- `internet/NATO_PZG_v2_4_1.vmod`: the downloaded NATO VASSAL module.
- `internet/NATO_PZG_v2_4_1.zip`: the downloaded VASSAL module archive.
- `internet/NATO_PZG_v2_4_1/`: the extracted VASSAL module, including `buildFile.xml`, maps, counters, scenarios, and other images.
- `internet/1985UAIS-4E-Rules-v48-LR-Flak-Review.pdf`: supplementary modern-warfare rules reference.
- `internet/1985_UAIS_(Thin_Red_Line).vmod`: supplementary VASSAL module.

`internet/` is intentionally ignored by Git. It is local research input rather than application source. Agents may inspect files in it, but must not rename, reorganize, edit, delete, or commit them unless the user explicitly requests that work. Do not make builds or tests depend on this directory being present in CI or in another developer's clone. If an expected resource is missing, report the missing path instead of inventing its contents.

## Current technology

- Tauri 2 desktop application shell.
- Rust backend and native integration under `src-tauri/`.
- React 19 user interface.
- TypeScript 6.
- Vite 8 development server and frontend build.
- npm for JavaScript package management.
- PixiJS 8 for the GPU-backed map renderer.

## Architecture direction

Game rules must eventually live in a UI-independent Rust core. Treat that core as the authoritative game state and rules implementation. It should be deterministic, testable without a window, serializable, and usable later by a local client, multiplayer server, AI, or replay tool.

### Tauri IPC boundary

Use a command-and-event boundary across Tauri IPC:

1. The frontend submits a player command such as `MoveUnit` or `ResolveCombat`.
2. Rust validates the command against the current state.
3. Rust updates the authoritative state and returns domain events or a state delta.
4. React and the map renderer present the result and play visual animations.

Do not duplicate rule validation in React. The frontend may calculate previews for responsiveness, but Rust remains authoritative. Avoid per-frame Tauri IPC; exchange commands, events, snapshots, and deltas at gameplay boundaries.

The shared protocol has four conceptual message types:

- **Snapshot**: a complete serializable game state used when starting a game, loading a save, reconnecting, or recovering from desynchronization.
- **Command**: a player's requested intent, such as `MoveUnit`, `DeclareAttack`, `ResolveCombat`, or `EndPhase`. A command is not proof that the action occurred.
- **Event**: an accepted domain fact produced by Rust, such as `UnitMoved`, `StepLost`, `SupplyStatusChanged`, or `PhaseEnded`.
- **Delta**: a compact group of changed state fields when an event stream alone is not convenient for updating the client.

The normal interaction is:

```text
React/PixiJS -- Command --> Rust game core
React/PixiJS <-- Events or Delta -- Rust game core
```

At initial load or after a detected mismatch:

```text
React/PixiJS -- snapshot request --> Rust game core
React/PixiJS <-- complete Snapshot -- Rust game core
```

Commands must use stable domain identifiers and game coordinates rather than screen coordinates. For example, a move command should contain a unit ID and destination hex, not the final mouse position. Rust must validate the active player, turn phase, ownership, movement allowance, terrain, path, stacking, zones of control, supply, airspace, and any other applicable rule before accepting it.

Pointer movement and visual animation stay in the frontend. During counter dragging, PixiJS updates the temporary screen position locally. Send one command to Rust when the player drops the counter on a destination hex. If Rust rejects the command, animate the counter back to its authoritative position. Do not send every pointer movement through IPC.

Do not poll or transfer the complete game state every frame. Do not use Tauri events as a substitute for the renderer's frame loop. IPC should occur at meaningful gameplay boundaries such as submitting an order, resolving an action, changing a phase, loading a save, or requesting an explicit resynchronization.

Maps, counter textures, fonts, audio, and other immutable assets should be loaded as application resources by the frontend or asset layer. Do not repeatedly send image bytes or base64 data through IPC. IPC messages should refer to assets by stable IDs or paths from an application-owned asset manifest.

Frontend previews are advisory. React or PixiJS may highlight reachable hexes or estimate combat odds for responsiveness, but submitting the action must cause Rust to recompute and validate the result. A preview must never mutate authoritative state.

Keep protocol types explicit and serializable. Prefer a versioned tagged representation that can be shared or generated between Rust and TypeScript. Return structured rejection reasons so the UI can explain why a command failed. Do not expose arbitrary internal Rust objects across IPC.

The intended ownership split is:

```text
Rust
  authoritative GameState
  rules and command validation
  combat resolution and seeded randomness
  save/load and replay
  Command -> Events/Delta

React
  HUD, dialogs, menus, logs
  input intent and local UI state

PixiJS or another map renderer
  map camera, counters, stacking visuals
  hit testing, paths, overlays, animation
```

Keep random outcomes reproducible through an explicit seeded random-number source. Saves and replays should not depend on wall-clock timing or UI state.

## Current repository layout

- `src/`: React and TypeScript frontend.
  - `main.tsx`: browser/WebView entry point.
  - `App.tsx`: current application component; presently the generated Tauri demo.
  - `App.css`: current demo styling.
- `public/`: files copied directly into the frontend build. Do not copy board-game
  counter images here; the frontend renders counters from structured unit data.
- `src-tauri/`: Rust and Tauri application.
  - `src/main.rs`: native executable entry point.
  - `src/lib.rs`: Tauri builder, current game session, and IPC commands.
  - `Cargo.toml`: Rust dependencies and build configuration.
  - `tauri.conf.json`: product, window, frontend build, and bundling configuration.
  - `capabilities/`: Tauri permissions.
  - `icons/`: platform application icons.
- `internet/`: ignored local reference material described above.
- `dist/`: generated Vite production output; do not edit.
- `node_modules/`: generated npm dependencies; do not edit.
- `src-tauri/target/`: generated Rust build artifacts; do not edit.
- `crates/ooaw-core/`: UI-independent Rust game core.
  - `src/model/`: units, sides, phases, scenario definitions, and scenario content.
  - `src/model/scenario_baltap.rs`: BALTAP 1983 setup and reinforcement schedule.
  - `src/engine.rs`, `src/state.rs`, and `src/phase.rs`: game-state execution at the crate root.
- `docs/`: durable design notes and decisions.
  - `docs/zh-CN/`: Chinese game-rule documentation.
  - `docs/en/`: matching English game-rule documentation.

Whenever a game rule is implemented or changed, update the corresponding documents in both `docs/zh-CN/` and `docs/en/`. Clearly distinguish implemented behavior from planned behavior.

Keep documentation categories strict:

- `docs/zh-CN/rules/` and `docs/en/rules/` are player-facing rulebooks. Write them for someone playing the game, using game terminology and instructions. Include only rules already implemented in code. Do not mention source code, APIs, commands, events, revisions, architecture, progress reports, roadmaps, or unimplemented behavior.
- `docs/zh-CN/develop/` and `docs/en/develop/` are developer-facing. They contain implementation status, architecture and protocol details, design decisions, unfinished work, and future plans.

## Development commands

Install frontend dependencies:

```bash
npm install
```

Run the complete desktop application in development mode:

```bash
npm run tauri dev
```

Run only the React/Vite frontend:

```bash
npm run dev
```

Validate the frontend production build:

```bash
npm run build
```

Validate the Rust side:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Build the integrated desktop executable without packaging an installer:

```bash
npm run tauri build -- --debug --no-bundle
```

## Multi-agent collaboration

Before starting work, inspect `git status`, this file, and the files relevant to the assigned task. The worktree may contain changes from another agent. Preserve them and do not use destructive Git commands to discard or rewrite work.

Keep each task within a clear ownership area. Good parallel work streams include:

- rules/domain model and deterministic tests in a future Rust game-core crate;
- reference-material analysis and design notes under `docs/`;
- React application shell and reusable HUD components under `src/`;
- map renderer experiments in an isolated frontend feature directory;
- asset-import tools in a separate `tools/` directory;
- save/replay format design without changing unrelated UI files.

Avoid having multiple agents make broad edits to `App.tsx`, `src-tauri/src/lib.rs`, `package.json`, or shared configuration at the same time. If a task requires one of these shared files, keep the change narrow and identify it clearly in the handoff.

Do not silently change the selected stack, introduce a second UI framework, or move gameplay authority into the frontend. Record lasting architectural decisions in `docs/`.

At handoff, report:

- the behavior or decision completed;
- every materially changed file;
- commands used for validation and their results;
- assumptions, unresolved questions, or follow-up work;
- any shared files that another agent should re-check before editing.

Run checks appropriate to the files changed. At minimum, frontend changes should pass `npm run build`, and Rust changes should pass `cargo check --manifest-path src-tauri/Cargo.toml`. Add focused tests for gameplay rules and serialization behavior when those systems are introduced.
