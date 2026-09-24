# Turn State Machine Development Notes

## Current implementation scope

Completed:

- independent `ooaw-core` Rust crate;
- war-agnostic side IDs;
- scenario-defined phase order;
- `new_game(scenario_id)`;
- `get_game_snapshot()`;
- `submit_game_command(request)`;
- the `EndPhase` command;
- continuous processing of automatic phases;
- revision checks;
- phase, game-turn, and game-completion events;
- tests covering the fourteen-turn state machine.

Not implemented yet:

- units, maps, and movement;
- resupply choices;
- battle-plan contents;
- strikes and ground combat;
- reserve movement;
- concrete automatic work for `preBattle` and `postBattle`;
- official scenario deployments and victory conditions.

## Confirmed design direction

The planned player turn is:

```text
Pre-battle
→ Battle planning
→ Offensive strike
→ Combat
→ Reserve
→ Post-battle
```

- `Pre-battle` will contain movement and recovery work that requires no player choice.
- The resupply choice from Recovery will move into `Battle planning`.
- `Battle planning` will use ruleset data to determine whether a plan is binding.
- `Combat` remains separate because strikes handle remote firepower while combat handles ground engagement, retreats, occupation, and advances.
- `Post-battle` will perform automatic cleanup such as unsuppression.

Sides, action order, phase presence, and phase ownership are scenario or ruleset data. They must not be constants in the state-machine engine.

The current prototype gives both sides a `battlePlanning` phase so that it can eventually hold both sides' resupply choices. Concrete rule handlers can distinguish binding and non-binding plans.

The complete game turn remains one flat sequence table rather than introducing a second state machine:

```text
jointStatus
→ jointReinforcement
→ warsawPact.preBattle ... warsawPact.postBattle
→ nato.preBattle ... nato.postBattle
```

Joint phases use actor `all`; later phases use the corresponding side as actor.

## Core module layout

The game state machine is split by responsibility while the crate root provides a stable public API:

```text
src/game/
├─ mod.rs       module organization and public re-exports
├─ state.rs     GameState, snapshots, and turn state
├─ command.rs   game commands and command outcomes
├─ event.rs     domain events
├─ error.rs     rule errors
├─ engine.rs    command execution and phase advancement
└─ tests.rs     state-machine tests
```

Callers continue to import these types through `ooaw_core::{...}` without depending on the internal file layout.

## Current IPC

Create a game:

```text
new_game("nato-1983-standard")
```

Current snapshot example:

```json
{
  "protocolVersion": 1,
  "gameId": "generated-uuid",
  "revision": 0,
  "scenario": {
    "id": "nato-1983-standard",
    "name": "NATO 1983 Rules Prototype",
    "maxGameTurns": 14,
    "sides": [
      { "id": "warsawPact", "name": "Warsaw Pact" },
      { "id": "nato", "name": "NATO" }
    ]
  },
  "status": "inProgress",
  "turn": {
    "gameTurn": 1,
    "stepIndex": 0,
    "currentStep": {
      "id": "joint.jointStatus",
      "phaseId": "jointStatus",
      "actor": { "type": "all" },
      "execution": "interactive"
    }
  },
  "pendingDecision": null
}
```

Current command request:

```json
{
  "expectedRevision": 0,
  "command": { "type": "endPhase" }
}
```

A stale revision produces `revisionMismatch`. An accepted command returns events and a fresh complete snapshot.
