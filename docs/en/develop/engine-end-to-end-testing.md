# Engine end-to-end testing

The kernel has a line-oriented JSON process adapter named `ooaw-engine`
(`crates/ooaw-core/src/bin/ooaw-engine.rs`). It runs games with the bundled
official rules plugin and is intended for automated clients, replay tools, and
black-box tests that must not link to Rust engine internals.

Each line written to standard input is one request. The process writes exactly
one JSON response line to standard output. It retains one game session until a
later `newGame` request replaces it or the process exits. Diagnostics are
written to standard error so standard output remains a stable protocol stream.

Supported requests are:

- `listScenarios`
- `newGame`, with a `scenarioId`, a deterministic `gameId`, and an optional `setup`
- `getSnapshot`
- `submitCommand`, with an `expectedRevision` and a normal serialized game
  command

Responses distinguish `gameStarted`, `snapshot`, `commandAccepted`,
`scenarios`, and `error`. Accepted commands include the new revision, emitted
events, and authoritative snapshot. Rejected commands do not increment the
revision. A caller must use the latest revision when submitting its next
command.

The integration test in `crates/ooaw-core/tests/engine_black_box.rs` launches
the compiled executable and communicates only with JSON. It verifies session
errors, the automatic opening resolved at game creation, rule rejection,
events and revisions, state mutation, and optimistic-concurrency rejection without importing any
`ooaw_core` type. A second test plays a BALTAP Warsaw Pact turn: it marks a
division OMG, checks that only that unit may move in the Reserve Phase and only
by Tactical movement, ends the phase (marker removal and Post-Battle), and
checks NATO's Pre-Battle supply report.

## Starting from a custom situation

`newGame` accepts an optional `setup` so a test can begin from any situation. The kernel creates the scenario and ends phases until it reaches `start` (running every phase on the way, including reinforcements and supply). It then passes the setup to the plugin that owns the scenario, which lays out the rest. For the NATO rules the format is `GameSetup` in `plugins/nato-official/src/setup.rs`. Every field is optional:

| Field | Effect |
| --- | --- |
| `start` | `{ gameTurn, phaseId, sideId? }`: the step to begin at; `sideId` is omitted for joint phases. Defaults to the first step. |
| `units` | Replaces every unit in play. Each entry is `{ id, hex?, step?, disruption?, supply?, trainStatus? }`. `id` is any scenario unit, including a reinforcement that has not arrived yet; omitting `hex` places it in the Strategic Reserve; `step` is the strength step index (0 = full); `supply` (`supplied` or `outOfSupply`) applies to every supply type the unit has and defaults to supplied. |
| `cityControl` | `[{ hexId, controller }]` overrides for city control. |
| `breakthroughMarkers` | Replaces the Breakthrough Markers. |
| `airInterdictionZones` | Replaces the zones: `[{ sideId, hexId }]`. |
| `airPoints` | Replaces the listed sides' Air Points: `[{ sideId, tactical, operational, bonusTactical }]`. |
| `attackTargets`, `reserveUnitIds` | Set the active battle plan's objectives and Reserve/OMG units; they need a battle plan, so `start` must be at or after that side's Battle Planning. |

The situation replaces state without re-checking rules, so tests can build positions ordinary play would take many turns to reach. Automatic work of the start step itself has already run: to have the engine check supply for a laid-out position, start at the previous side's Reserve Phase and submit `endPhase`. An unknown unit, hex, side, or field, or an unreachable `start`, is rejected with `invalidSetup`. No game is started by a rejected request.

The black-box tests use this to surround a West German brigade with three Soviet divisions and check that NATO's next Pre-Battle step puts it out of supply, and to check the rejections.

Run it with:

```sh
cargo test -p ooaw-core --test engine_black_box
```

Example session:

```json
{"type":"newGame","scenarioId":"nato-1983-standard","gameId":"example"}
{"type":"newGame","scenarioId":"nato-baltap-1983","gameId":"pocket","setup":{"start":{"gameTurn":1,"sideId":"warsawPact","phaseId":"reserve"},"units":[{"id":"westGermany.6panzergrenadierDivision.16panzergrenadierBrigade","hex":"2617"},{"id":"soviet.2gta.21motorRifleDivision","hex":"2516"}]}}
{"type":"submitCommand","expectedRevision":0,"command":{"type":"endPhase"}}
{"type":"getSnapshot"}
```
