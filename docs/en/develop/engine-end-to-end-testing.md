# Engine end-to-end testing

The engine has a line-oriented JSON process adapter named `ooaw-engine`. It is
intended for automated clients, replay tools, and black-box tests that must not
link to Rust engine internals.

Each line written to standard input is one request. The process writes exactly
one JSON response line to standard output. It retains one game session until a
later `newGame` request replaces it or the process exits. Diagnostics are
written to standard error so standard output remains a stable protocol stream.

Supported requests are:

- `listScenarios`
- `newGame`, with a `scenarioId` and deterministic `gameId`
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
errors, rule rejection, automatic phase processing, reinforcement events,
state mutation, and optimistic-concurrency rejection without importing any
`ooaw_core` type.

Run it with:

```sh
cargo test --manifest-path crates/ooaw-core/Cargo.toml --test engine_black_box
```

Example session:

```json
{"type":"newGame","scenarioId":"nato-1983-standard","gameId":"example"}
{"type":"submitCommand","expectedRevision":0,"command":{"type":"endPhase"}}
{"type":"getSnapshot"}
```
