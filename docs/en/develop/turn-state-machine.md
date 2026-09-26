# Turn State Machine Development Notes

## Current implementation scope

Completed:

- independent `ooaw-core` Rust crate;
- war-agnostic side IDs;
- scenario-defined phase order;
- `new_game(scenario_id)`;
- `list_scenarios()`;
- `get_game_snapshot()`;
- `submit_game_command(request)`;
- the `EndPhase` command;
- continuous processing of automatic phases;
- revision checks;
- phase, game-turn, and game-completion events;
- generic unit, strength-step, and map-location data structures;
- scenario-defined reinforcement schedules by game turn;
- automatic Joint Reinforcement resolution and `reinforcementsArrived` events;
- a minimal opening deployment represented as turn-one reinforcements;
- the seven-turn BALTAP 1983 framework, 44 units, opening setup, and reinforcement schedule;
- structured unit and strength data from which the frontend can draw counters;
- tests covering both seven-turn and fourteen-turn state machines.

Not implemented yet:

- the complete unit roster, map rules, and movement;
- resupply choices;
- battle-plan contents;
- strikes and ground combat;
- reserve movement;
- concrete automatic work for `preBattle` and `postBattle`;
- deployments and victory conditions for the remaining official scenarios;
- BALTAP special rules and victory conditions;

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

`jointReinforcement` is automatic. On entering it, the core selects units scheduled for the current turn, adds them to authoritative state, and returns a `reinforcementsArrived` event containing their complete state. Turn-one entries use the same mechanism for opening deployment, so there is no separate setup path.

## Core module layout

The game state machine is split by responsibility while the crate root provides a stable public API:

```text
src/model/
├─ mod.rs               model organization and public re-exports
├─ side.rs              side identifiers and definitions
├─ phase.rs             phase identifiers, execution, and actors
├─ scenario.rs          scenario model, registry, and shared builders
├─ scenario_baltap.rs   BALTAP scenario content
└─ unit.rs              unit identifiers, definitions, steps, and locations

src/
├─ state.rs     GameState, snapshots, and turn state
├─ command.rs   game commands and command outcomes
├─ event.rs     domain events
├─ error.rs     rule errors
├─ engine.rs    command execution and phase advancement
├─ phase.rs     phase-entry handlers
└─ tests.rs     state-machine tests
```

`src/model/` contains game-content data models, while the crate root contains command execution and state-machine behavior. `src/model/unit.rs` defines stable unit IDs, nationality, unit type, formation, strength steps, map location, and runtime unit state. Unimplemented phases have empty handlers in the root `phase.rs`, ready to be filled without changing the outer state machine.

`src/model/scenario_baltap.rs` contains BALTAP 1983 scenario data. It defines 44 units: 27 on turn one, followed by 11, 2, 2, 1, and 1 on turns two through six. Of the turn-one units, 17 enter map hexes and 10 enter the Strategic Reserve. Later reinforcements enter the Strategic Reserve.

Counters do not depend on image assets. The core supplies game information through the unit definition and structured `attack`, `defense`, and `movement` fields for each strength step, and the frontend draws counters in its own visual style.

Models and scenario queries are available through `ooaw_core::model::{...}`. The crate root continues to re-export the public items, so existing `ooaw_core::{...}` imports remain compatible.

## Current IPC

Create a game:

```text
new_game("nato-1983-standard")
```

List available scenarios:

```text
list_scenarios()
```

The BALTAP 1983 scenario ID is:

```text
new_game("nato-baltap-1983")
```

Current snapshot example:

```json
{
  "protocolVersion": 4,
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
  "units": [],
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

After the client ends turn one's `jointStatus`, the state machine enters and automatically completes `jointReinforcement`. The returned event stream includes:

```json
{
  "type": "reinforcementsArrived",
  "gameTurn": 1,
  "units": [
    {
      "id": "soviet.6thGuardsMotorRifleDivision",
      "name": "6th Guards Motor Rifle Division",
      "sideId": "warsawPact",
      "nationId": "sovietUnion",
      "unitTypeId": "motorRifleDivision",
      "formationId": null,
      "traits": [],
      "steps": [
        {
          "attack": 8,
          "defense": 6,
          "movement": 5
        },
        {
          "attack": 4,
          "defense": 4,
          "movement": 5
        }
      ],
      "strengthStepIndex": 0,
      "location": { "type": "hex", "hexId": "2806" }
    }
  ]
}
```

The actual event contains every unit arriving that turn; the example shows one. The complete snapshot returned with it also lists every unit currently in play under `units`.
