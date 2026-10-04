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
- six year variants of the three campaign scenarios, year-specific units and war-turn arrivals (scope in [Campaign scenarios](campaign-scenarios.md));
- structured unit and strength data from which the frontend can draw counters;
- an authoritative map model and NATO map data owned by each `ScenarioDefinition`;
- one-time map delivery when `new_game` creates a game, with ordinary snapshots retaining only a stable `mapId`;
- a complete frontend command path that submits `EndPhase` and consumes returned events and authoritative snapshots;
- on-map units, Strategic Reserve totals, and programmatically drawn counters after opening reinforcement resolution;
- unit HQ, movement, and combat supply state;
- automatic `preBattle` resolution and `preBattleSupplyChecked` events;
- freely interleaved battle-planning commands for resupply selection, attack objectives, tactical/march movement, entraining/detraining, rail movement, and air transport;
- authoritative pathfinding and validation for terrain and hexside costs, movement allowance, prohibited terrain, enemy zones of control, minimum movement, the Danish Ferry, enemy occupation, and stacking, plus a read-only movement preview;
- scenario rule data for the 20-hex rail limit, eight-step WP and ten-step NATO rail capacities (Entrained units only), per-side Airlift Commands (BALTAP: WP 3, NATO 1), the number of resupply operations per turn, and the four-maneuver-step stacking limit;
- frontend map overlays for planned routes and attack objectives;
- Airspace (rule 11) and the Offensive Strike Phase's Air Strike Segment: Air Points, Air Strikes, Air Interdiction Zones, Disrupted/Suppressed markers, step loss, elimination, and Breakthrough Markers, with seeded dice;
- Reserve/OMG status, the Reserve Phase with Breakthrough Zones, and Post-Battle unsuppression (see "Reserve and Post-Battle Phases");
- simplified supply traced in each side's Pre-Battle step (see "Simplified supply");
- tests covering both seven-turn and fourteen-turn state machines.

Not implemented yet:

- the complete unit roster and road/river movement modifiers not yet represented in map data;
- the full rule-10 supply model (home and friendly countries, Superior HQ assignments, ports, map-edge sources, city supply chains, and Combat Supply re-checked after each battle); a simplified two-alliance version is implemented instead;
- interception, air-transport losses, and the complete Lift Command order of battle;
- Nuclear, Chemical, and Artillery Strikes, NATO Deep Interdiction, NATO Defensive Air Strikes, and the parts of ground combat listed under "Combat Phase" below;
- Reserve Helicopter movement (28.2.2), which needs helicopter movement;
- complete campaign deployment, pre-war and peace/mobilization sequences, and official scenario victory conditions (six war-turn entries are available; see [Campaign scenarios](campaign-scenarios.md));
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

- `Pre-battle` is automatic. It resolves HQ and movement supply state before player actions and requires no player input.
- Resupply operations, rail/air/sea/ground/helicopter movement, reserve designation, and post-movement recovery will move into `Battle planning`.
- The choice of how to use a resupply marker during Recovery will also move into `Battle planning`.
- `Battle planning` will use ruleset data to determine whether a plan is binding.
- `Combat` remains separate because strikes handle remote firepower while combat handles ground engagement, retreats, occupation, and advances.
- `Post-battle` will perform automatic cleanup such as unsuppression.

Sides, action order, phase presence, and phase ownership are scenario or ruleset data. They must not be constants in the state-machine engine.

The current prototype gives both sides a `battlePlanning` phase so that it can eventually hold both sides' resupply choices. Concrete rule handlers can distinguish binding and non-binding plans.

`preBattle` checks the supply of every unit of the acting side with the simplified rule below and records the result in `preBattleSupplyChecked`. Reinforcements enter play fully supplied, and units in the Strategic Reserve are always supplied.

`battlePlanning` is now the acting side's unified command phase. Resupply selection, adding or removing attack objectives, moving another unit, entraining, detraining, and air transport have no artificial submission order; each command checks only its real rule prerequisites. Movement immediately updates the authoritative location and records the route selected by the core. Objectives and resupply targets remain in `battlePlan`. Ending the phase restores applicable supply to the acting side's combat units in each selected target's hex, while the plan remains available to later strike and combat phases.

Ground movement supports Tactical and March modes; see the player rules in `docs/en/rules/movement.md` and the implementation notes in "Movement rules" below. Entraining consumes the current planning phase and changes to `entrained` when that side next begins Battle Planning; an entrained unit may travel at most 20 hexes per turn. Basic Air Transport is limited to Airborne/Airmobile units carrying the `airTransportable` trait and moves them from a city or Strategic Reserve to a legal non-sea, non-mountain hex outside enemy ZOCs. Sea transport remains deferred.

### Movement rules

`src/movement.rs` holds all movement rules. `moveUnit` and the read-only `movement_options` query share one search and one set of destination checks, so every previewed destination is accepted as an order. The frontend applies no movement rules: which movement systems a unit may use, why the others are unavailable, and every legal destination come from `GameEngine::movement_modes`. Numbers in parentheses refer to the 2020 rulebook (`internet/NATO_Rules_2020.pdf`).

Implemented:

- terrain cost by primary terrain, with a city outranking the terrain beneath it (2.2.1); a city costs 1 (12.3 example);
- Major River hexside +1 (12.1 example);
- Prohibited Terrain: All-Sea hexes, All-Sea hexsides without a Causeway, Blocked hexsides, enemy-occupied hexes (12.1.1, 12.1.2);
- ZOC projection: attack 1+ and HQs project into six adjacent hexes, attack 0 only into their own hex, none under a train marker, across Blocked and All-Sea hexsides (8.0, 13.4.1);
- Tactical ZOC costs of +1 to enter and +1 to leave, the Soft-unit stop rule (also enforced across split orders), and Soft EZOC-to-EZOC moves only into friendly-occupied hexes (12.3, 12.3.1, 12.3.2);
- March: double allowance; not for HQs or units Out of Movement Supply; may not start in, enter, or end in an EZOC (12.4, 8.1);
- Minimum movement for units that have not moved, computed from the direct single-hex edge (12.5);
- the Danish Ferry, 1513/1514: one NATO unit per direction per phase, as Minimum movement (12.8);
- Rail: 20 hexes, no Prohibited Terrain, may not leave or enter an EZOC (13.2);
- Stacking: four Maneuver steps plus one HQ at the end of each order (9.1.1);
- Cities (30): ground movement never enters an enemy Free City; only Tactical movement enters an enemy Conquered City, taking control of every one it passes through (liberating it if the mover is the original owner); March and Rail avoid enemy-controlled cities; air transport never ends in one; a friendly Free City negates EZOCs for Soft-unit entry like a friendly unit (30.1.1, 30.2.1–30.2.3, 30.3.1, 30.3.2).

Not verified or not implemented:

- The TEC is printed on the back of the Sequence of Play cards. It is not in `internet/`: not in the VASSAL module (`internet/NATO_PZG_v2_4_1/`, identical to the copies in `~/Downloads`), the 2020 rulebook, or the BoardGameGeek living rules and play booklet (`internet/NATO_Living_Rules_Booklet_1-1-26.pdf`, `internet/NATO_Play_Booklet_Updated_1-1-26.pdf`). The board's Terrain Key gives only terrain priorities (Key/Major City 1, Minor City 2, Mountain 3, Rough 4, Forest 5, Marsh 6, Clear 7). Marsh costs 1: the Designer's Note to 25.8.2 says Marsh hexes are no different from Clear except that they block Exploitation. Rough and Mountain costs (2/3), a cost of 1 for Major and Key cities, and a Minor River cost of 0 (`BattlePlanningRules::minor_river_cost`) remain placeholders; the Mountain Pass hexside is not modelled.
- There is no Territorial home-country limit (12.7), no OMG/Reserve markers (12.6), no Pass hexsides, and no Refugees. Airspace ignores the Danish exception (11.4) because hex nationality is not in the map data.
- The Warsaw Pact cannot use the Danish Ferry because Danish surrender is not modeled.
- BALTAP play area (Play Booklet 36.4.1.2): no unit may move or trace supply south of the Elbe. It is not enforced yet: the traced river hexsides do not form a closed boundary, so the excluded hexes must be defined from the board by hand.

Living rules (1 Jan 2026) errata applied: rail capacity counts only Entrained units (13.3); Entraining units flip to Entrained at the start of Battle Planning only while capacity allows, in unit-ID order (a player choice of which units flip is not yet offered); air transport may not start in an EZOC (16.1.1); BALTAP Airlift Commands are WP 3 and NATO 1, each carrying one step per turn (3.8, 36.2). Other 1-1-26 changes (OMG/Reserve eligibility in 12.6, WP battle commitment in 25.1.1.1) concern systems that are not implemented.
- Stacking is checked after each order rather than only at the end of the phase (9.1.1 allows passing through).

Hexside data: `allSea`, `causeway`, `majorRiver`, `minorRiver`, and `danishFerry` in `crates/ooaw-core/data/natoMap.json` were derived by the local, uncommitted tool `tmp/map-extract/water_hexsides.py`. A hexside is All-Sea when the sea colour covers a band across at least 85% of its length and no traced river runs along it. The river kind comes from the traced river polylines. The ferry is set from rule 12.8. Only one traced causeway (the Little Belt bridge) exists, and it crosses a hexside that is not All-Sea; the Afsluitdijk and other causeways are not yet traced.

Every planning order can be reversed during the same plan: `setResupplyTarget` with `selected: false` removes that resupply target, `setAttackTarget` with `selected: false` removes an objective, `undoUnitMovement` reverses a unit's latest movement leg, `detrainUnit` cancels an entrainment order given in this plan, and `undoDetrainUnit` restores the Entrained marker of a unit detrained in this plan. `undoDetrainUnit` is rejected if the unit has since used non-rail movement or if rail capacity is no longer available.

The complete game turn remains one flat sequence table rather than introducing a second state machine:

```text
jointStatus
→ jointReinforcement
→ warsawPact.preBattle ... warsawPact.postBattle
→ nato.preBattle ... nato.postBattle
```

Joint phases use actor `all`; later phases use the corresponding side as actor.

`jointReinforcement` is automatic. On entering it, the core selects units scheduled for the current turn, adds them to authoritative state, and returns a `reinforcementsArrived` event containing their complete state. Turn-one entries use the same mechanism for opening deployment, so there is no separate setup path.

### Offensive Strike Phase

`src/strikes.rs` implements the Air Strike Segment (20, 23), `src/airspace.rs` computes Airspace (11), and `src/dice.rs` provides a SplitMix64 die seeded from the game ID, so a game replays identically for the same commands. The dice state is part of `GameEngine` but not of the client snapshot; saves must persist it.

- **Air Points** reset each Joint Reinforcement Phase from `BattlePlanningRules::air_power` (per side: Tactical and Operational per turn and a one-time bonus Tactical point). BALTAP: 1 Tactical per turn each, 1 bonus each, no Operational, Surprise on turn 1 (Play Booklet 36.4.1.4, 36.4.1.6). The rules prototype uses placeholder values (2 Tactical, 1 Operational); the Air Campaign Table is not implemented.
- **Missions** (`strikePlan.missions`) are committed with `planAirStrike` or `planAirInterdiction` and resolved together with `resolveAirStrikes`, or automatically by `endPhase` (23.2.4). `cancelAirMission` refunds a pending mission to the pool it came from. Targeting enforces: two steps per strike, HQs alone and only with Operational points, no unit struck twice, two strikes per hex, and Tactical points only in friendly or contested Airspace.
- **Strike Table**, one-point column, with the printed modifiers: Major/Key City −2; Forest, Rough, Mountain, or Minor City −1; train marker +1 instead of terrain; friendly Airspace +1; enemy Airspace −1; WP on the Surprise turn +1. When targets differ, the lowest total applies (23.3.1). The first named unit absorbs a step loss.
- **Results**: Disrupted (HQs Suppressed; a Disrupted unit loses its train marker), step loss (flip and Disrupt, or eliminate), and a Breakthrough Marker when the last enemy unit in the hex is eliminated.
- **Marker timing** in the merged sequence: Disrupted markers are removed when their side's Battle Planning ends (the original Recovery Phase follows movement); Suppressed markers are removed in their side's Post-Battle step (Unsuppression); Breakthrough Markers and enemy Air Interdiction Zones are removed when the acting side ends its Reserve Phase (23.8.2, 28.2.5).
- **Airspace**: each side projects within five hexes of its supplied on-map units and of every city it controls, except West Berlin (11.5, the map's `contestsAirspace: false`). City supply is not traced, so all controlled cities count. A hex projected by neither side is treated as contested. Airspace now also governs March and Rail (friendly only), entraining (friendly only), and Air Transport (not from or into enemy Airspace).
- **Air Interdiction Zones** (23.8) add +1 MP to the other side's Tactical movement entering the zone and bar its March and Rail movement.
- **Disruption and movement**: Disrupted or Suppressed units may use only Minimum movement and may not entrain.

Not implemented in this phase: Nuclear and Chemical Strikes (with Armageddon and war-crimes penalties), WP Artillery divisions (BALTAP has none), NATO Deep Interdiction, strikes on Reforger Sites, Fortified hexes, and the Danish Airspace exception.

### Combat Phase

`src/combat.rs` implements a simplified Combat Phase (25) in which only the Attacker makes choices. The WP must attack every Battle Marker objective it still can (`mandatoryAttacksRemaining` rejects `endPhase` until then). NATO may attack any eligible hex.

- **Commands:** `resolveBattle { hexId, unitIds, supportingHqId? }` commits adjacent Maneuver units, optionally with one HQ's Offensive Support, and resolves the battle at once. `advanceAfterCombat { unitIds }` advances survivors into a cleared hex, or declines with an empty list. While an advance is pending, the snapshot's `pendingDecision.kind` is `advanceAfterCombat`, no other battle may start, and `endPhase` declines it.
- **Strengths** are computed in sixty-fourths so fractions stay exact.
  - Attack: Disrupted ½, Out of Combat Supply ½, Armored into a City or Mountain ½, Minor River ¾, Major River ½.
  - Defense: Disrupted ½, Out of Combat Supply ½, Soft units in cover ×2, the Free City's Organic Defense, and a Provisional HQ only when no Maneuver unit defends.
  - Units under a train marker and Engaged units add nothing.
- **Offensive Support** (25.4): `ScenarioDefinition::offensive_support_hqs` lists each HQ able to support and its Subordinate formations. The HQ's first-step Attack value is its Support Range (3.4.1). It must be supplied, not Suppressed, not under a train marker, and unused this phase (`combat.supportingHqIds`). A breadth-first search from the HQ must reach a committed Subordinate unit within the range, not through enemy units, enemy-controlled cities, unnegated EZOC hexes, All-Sea hexes, or Blocked/All-Sea hexsides (the Danish Ferry excepted). `combat_options` lists `supportHqIds` per objective, assuming every eligible unit attacks. `battle_preview` and `resolveBattle` re-validate against the actual commitment.
- **BALTAP** (36.4.2.4, 36.4.2.6): the NEGF HQ supports the NEGF and 2nd Guards Tank Army formations. It carries the `immobile` trait (all movement and entraining rejected with `unitImmobile`) and is removed at the start of Game Turn 4 through `ScenarioDefinition::withdrawals` (`unitWithdrawn` event).
- **Odds:** the attack total rounds down and the defense total rounds up; odds below 1:1 round in the Defender's favour. Column shifts come from terrain, Flank (+1) and Concentric (+2, map edges never count as surrounded), Offensive Support (+1), and Surprise (+1 WP). They are capped at ±2, except the WP's upward shifts on the Surprise turn. The CRT and Counterattack Table are transcribed from the map.
- **Automated Defender decisions:**
  - defending losses: Maneuver units first, strongest first; HQs are Suppressed instead;
  - Counterattack targets: the strongest attacker not yet Disrupted; NATO Counterattacks with its most numerous nationality;
  - the attacking step lost to A1: the strongest attacker;
  - the Defender never trades steps to shorten a retreat;
  - the retreat route follows the 25.7.4 priorities, with "toward the friendly rear" approximated as farthest from the attackers.
- **Results** (25.6): steps lost, retreats with EZOC and unfulfilled-hex losses, Disruption, Engaged markers, Consolidation advance up to the stacking limit, city capture by advancing, and Breakthrough Markers after the advance decision. An empty Breakthrough Marker hex is an advance-only battle.

Not implemented: Attack Helicopters, Defender Reaction, NATO Defensive Strikes, Exploitation, Assaults, Coordination (26), the WP multi-Front Counterattack restriction, the Defender's choice of losses and retreat and the "They shall not pass!" option, the Joint Combat Supply check, and the true friendly-rear retreat direction.

### Reserve and Post-Battle Phases

`src/reserve.rs` implements Reserve/OMG status (12.6, living rules of 1 Jan 2026) and the Reserve Phase (28.2).

- **Marking:** `setReserve { unitId, selected }` during Battle Planning stores the unit in `battlePlan.reserveUnitIds`. A unit qualifies when it is a Maneuver unit on the map, not immobile, not Disrupted, not Out of Movement or Combat Supply, not under a train marker, has not used rail or air transport this plan, has spent at most half its allowance (Tactical ≤ ⌊MA/2⌋, March ≤ MA, so Minimum movement never qualifies), and is not in an EZOC. `reserve_options` previews the result for every planning-side unit.
- **Re-check:** after a move, undo, or entrain order of a marked unit, the core re-checks it. A unit that no longer qualifies loses the marker with `reserveStatusChanged { selected: false }`, instead of the order being rejected.
- **Combat:** marked units are left out of `combat_options`, so they cannot be committed.
- **Reserve Phase:** entering it opens `snapshot.reserve { sideId, unitIds, movements }` with the surviving marked units. `moveUnit` and `undoUnitMovement` work as in planning but record into `reserve.movements`. Only listed units may move (`notInReserve`) and only by Tactical movement (`reserveTacticalOnly`); a single-hex Minimum move stays available as the first move. The allowance is ⌈printed MA / 2⌉. For Hard units, EZOC hexes inside a Breakthrough Zone (the marker hex and its six neighbours) do not add the +1 to enter or leave (25.9.3, 28.2.1).
- **End of the Reserve Phase:** `endPhase` removes the side's Breakthrough Markers and the enemy's Air Interdiction Zones, then all Reserve/OMG markers (`reserveMarkersRemoved`), and clears `snapshot.reserve`.
- **Post-Battle** (7.2 H, Unsuppression) is automatic: it removes the acting side's Suppressed markers (HQs), emitting `unitDisruptionChanged { disruption: null }`. The rulebook's phase does nothing else.

Section 28.1 still says a reserve unit may not be *adjacent* to an enemy unit; the living rules' 12.6 says it may not *end its movement in an EZOC*. The core follows 12.6, since the living rules take precedence.

### Simplified supply

`src/supply.rs` replaces rule 10 with a house rule that knows only the two alliances, so no per-hex country data is needed.

- **Line of Supply** (`los_distances`, shared with Offensive Support): a breadth-first search that never enters enemy units, enemy-controlled cities, All-Sea hexes, or EZOC hexes without a friendly unit or friendly Free City, and never crosses Blocked or All-Sea hexsides. For supply, only NATO may cross the Danish Ferry; Offensive Support keeps its existing ferry exception.
- **HQs** are checked first: supplied if a LOS of up to 10 hexes reaches any city the side controls (Free or Conquered). A supplied HQ not under a train marker supplies combat units within its Support Range (first-step Attack value). Suppressed HQs still supply.
- **Combat units** are supplied if a LOS reaches a friendly Free City within 10 hexes or a supplying HQ within its range. Any friendly HQ supplies any friendly unit.
- **West Berlin** carries `enclave: true` in the map data: it supplies only units at distance 0 or 1 and never HQs.
- The result sets HQ supply, or both Movement and Combat Supply, and emits `unitSupplyChanged` for each unit whose state changed. Existing effects then apply: unsupplied units get half Tactical movement and no March, rail, air transport, or Reserve marker, and fight at half strength; unsupplied HQs move at half allowance and cannot entrain or give Offensive Support. Only supplied units project Airspace.

Deliberate differences from rule 10: no countries (so a unit inside enemy territory can still draw on a Free City within 10 hexes), no city or port supply, no Superior HQ assignments, no map-edge sources, and Combat Supply is not re-checked after each battle.

### Reinforcement arrival and lift movement (house rules)

Reinforcement Boxes are replaced by one entry hex per Reinforcement Sector: the land map-edge hex nearest each printed box (`MapDefinition::reinforcement_sectors`, set in code: NATO 1 = 3534, 2 = 4734; WP 3 = 4301, 4 = 3301, 5 = 2501).

- **Arrival** (`GameEngine::arrival_location` in `phase.rs`): from turn two, a unit due in a map hex takes the nearest hex with stacking room within three hexes; a unit due in an enemy-held hex (enemy units or an enemy-controlled city), or with no room nearby, enters the Strategic Reserve. Turn-one units are the setup and are placed as written. A unit defined Entrained arrives Entrained while rail capacity allows (13.6), otherwise Entraining.
- **Air Transport** (`airTransport`): Airborne or Airmobile units fly from a city or the Strategic Reserve to a city their side controls. The route (`lift_search`) passes any hex outside enemy Airspace, EZOCs included; the destination is not in enemy Airspace, an EZOC, or Mountain terrain. Take-off follows 16.1.1: Movement Supply, no train marker, not Disrupted, not moved, origin not in enemy Airspace, an EZOC (unnegated), or Mountain. Units in the Strategic Reserve start from their side's sector entry hexes.
- **Paradrop** (`paradrop`, 16.1.3): Airborne-trait units only; any Clear or Marsh hex, including enemy Airspace and EZOCs, never an enemy unit (Assaults are not implemented) or an enemy-controlled city; no route restriction.
- **Sea Transport** (`seaTransport`): any unit from a port or the Strategic Reserve to a port its side controls, along All-Sea, Coastal, port, and Major River hexes outside enemy Airspace, a river hex not in an EZOC (approximating 16.2.1.1); the destination is not in enemy Airspace or an EZOC. Units in the Strategic Reserve start from All-Sea hexes on their side's map edge.
- **Capacity**: Air Transport and Paradrop share the Airlift Commands (`airliftStepsUsed`); Sea Transport uses the scenario's Sealift Commands (`sealiftStepsUsed`; BALTAP WP 3 / NATO 2, campaigns 3 / 3). Rejections use `airliftCapacityExceeded` and `sealiftCapacityExceeded`.
- Units may no longer entrain in the Strategic Reserve. Interception (17.0), Air/Sea Ferry as separate systems, Amphibious Operations, and Helicopter movement are not implemented.

## Core module layout

The game state machine is split by responsibility while the crate root provides a stable public API:

```text
src/model/
├─ mod.rs               model organization and public re-exports
├─ map.rs               map, terrain, city, grid, and hexside models
├─ planning.rs          battle plans, objectives, and movement records
├─ strike.rs            Air Points, air missions, Airspace, Disruption, strike previews
├─ combat.rs            battle reports, odds, CRT results, combat previews
├─ side.rs              side identifiers and definitions
├─ phase.rs             phase identifiers, execution, and actors
├─ rules.rs             movement modes, train status, and scenario rule parameters
├─ scenario.rs          scenario model, registry, and shared builders
├─ scenario_baltap.rs   BALTAP scenario content
└─ unit.rs              unit identifiers, definitions, steps, and locations

src/
├─ engine.rs    GameEngine, snapshots, turn state, and command execution
├─ command.rs   game commands and command outcomes
├─ event.rs     domain events
├─ error.rs     rule errors
├─ phase.rs     phase-entry handlers
├─ planning.rs  battle-planning commands and plan bookkeeping
├─ strikes.rs   Offensive Strike Phase: air missions, Strike Table, markers
├─ combat.rs    Combat Phase: odds, CRT, results, retreat, advance
├─ reserve.rs   Reserve/OMG status and the Reserve Phase
├─ supply.rs    simplified supply and Line of Supply tracing
├─ setup.rs     custom starting situations (GameSetup) for tests
├─ airspace.rs  Airspace from each side's point of view
├─ dice.rs      seeded, reproducible dice
├─ movement.rs  movement rules, pathfinding, and movement previews
└─ tests.rs     state-machine tests
```

`src/model/` contains game-content data models, while the crate root contains command execution and state-machine behavior. `src/model/map.rs` defines the serializable map structure and loads the embedded NATO map from `crates/ooaw-core/data/natoMap.json`. `src/model/unit.rs` defines stable unit IDs, nationality, unit type, formation, strength steps, map location, runtime unit state, and HQ, movement, and combat supply state. `src/planning.rs` implements battle-planning commands, and `src/movement.rs` implements authoritative movement rules and pathfinding; the other unimplemented phases retain empty handlers in the root `phase.rs`.

`src/model/scenario_baltap.rs` contains BALTAP 1983 scenario data. It defines 44 units: 27 on turn one, followed by 11, 2, 2, 1, and 1 on turns two through six. Of the turn-one units, 17 enter map hexes and 10 enter the Strategic Reserve. Later reinforcements enter the Strategic Reserve.

Counters do not depend on image assets. The core supplies game information through the unit definition and structured `attack`, `defense`, and `movement` fields for each strength step, and the frontend draws counters in its own visual style.

The frontend calls `new_game` and `submit_game_command` through `src/gameApi.ts`. Joint Status is automatic (it has no player decisions yet), so `GameEngine::new` resolves the scenario's leading automatic phases (Joint Status, Joint Reinforcement with the opening deployment, and the WP Pre-Battle) before returning. Play opens on WP Battle Planning at revision 0 with the opening forces on the map; the events of that automatic opening are not returned. After each command the frontend redraws its unit layer and battle-plan overlay from the returned snapshot and focuses the camera on arriving reinforcements. Counters are drawn from PixiJS rectangles, lines, ellipses, and text; no counter images from the reference game are loaded.

Models and scenario queries are available through `ooaw_core::model::{...}`. The crate root continues to re-export the public items, so existing `ooaw_core::{...}` imports remain compatible.

## Current IPC

Create a game:

```text
new_game("nato-1983-standard")
```

`new_game` returns bootstrap data:

```json
{
  "snapshot": { "protocolVersion": 17, "scenario": { "mapId": "nato-central-europe" }, "battlePlan": null },
  "map": {
    "id": "nato-central-europe",
    "version": 1,
    "grid": {},
    "hexes": [],
    "hexsides": []
  },
  "turnSequence": [
    { "id": "joint.jointStatus", "phaseId": "jointStatus", "actor": { "type": "all" }, "execution": "automatic" },
    { "id": "warsawPact.battlePlanning", "phaseId": "battlePlanning", "actor": { "type": "side", "sideId": "warsawPact" }, "execution": "interactive" }
  ]
}
```

The example omits the full grid, terrain, and rendering data. The map and the scenario's `turnSequence` (the ordered steps of one game turn, from `GameEngine::turn_sequence`) are sent once in the game-creation bootstrap; snapshots returned by later commands do not repeat them and locate the current step by `turn.stepIndex`.

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
  "protocolVersion": 17,
  "gameId": "generated-uuid",
  "revision": 0,
  "scenario": {
    "id": "nato-1983-standard",
    "name": "NATO 1983 Rules Prototype",
    "maxGameTurns": 14,
    "mapId": "nato-central-europe",
    "sides": [
      { "id": "warsawPact", "name": "Warsaw Pact" },
      { "id": "nato", "name": "NATO" }
    ]
  },
  "status": "inProgress",
  "turn": {
    "gameTurn": 1,
    "stepIndex": 3,
    "currentStep": {
      "id": "warsawPact.battlePlanning",
      "phaseId": "battlePlanning",
      "actor": { "type": "side", "sideId": "warsawPact" },
      "execution": "interactive"
    }
  },
  "units": [ /* the opening forces */ ],
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

Read-only planning previews (they never change state or the revision):

```text
movement_options({ "request": { "unitId": "soviet.2gta.21motorRifleDivision" } })
```

```json
{ "revision": 3, "unitId": "soviet.2gta.21motorRifleDivision",
  "modes": [
    { "mode": "tactical", "unavailable": null,
      "options": [{ "hexId": "2411", "cost": 1, "path": ["2411"] }] },
    { "mode": "rail", "options": [],
      "unavailable": { "code": "unitNotEntrained", "message": "A unit must finish entraining before it can move by rail" } }
  ] }
```

```text
attack_target_options()  →  { "revision": 3, "hexIds": ["2214", "2415"] }
```

`attack_target_options` lists hexes containing an enemy unit or an enemy Free City (25.1.1).

```text
air_strike_options()  →  { "revision": 5, "tacticalHexes": [...], "friendlyHexes": [...],
  "targets": [{ "hexId": "2415", "airspace": "contested", "tacticalAllowed": true, "strikesRemaining": 2,
                "units": [{ "unitId": "...", "steps": 1, "modifier": 1, "headquarters": false, "alreadyTargeted": false }] }] }
```

Offensive Strike commands:

```json
{ "type": "planAirStrike", "hexId": "2415", "unitIds": ["westGermany.6panzergrenadierDivision.16panzergrenadierBrigade"], "airPoint": "tactical" }
{ "type": "planAirInterdiction", "hexId": "2414", "airPoint": "tactical" }
{ "type": "cancelAirMission", "missionId": 1 }
{ "type": "resolveAirStrikes" }
```

Combat queries and commands:

```text
combat_options()  →  { "revision": 9, "mandatoryRemaining": ["2415"],
  "objectives": [{ "hexId": "2415", "eligibleUnitIds": ["..."], "mandatory": true, "breakthroughOnly": false }] }
battle_preview({ "request": { "hexId": "2415", "unitIds": ["..."], "supportingHqId": null } })  →  { "revision": 9, "odds": { "totalAttack": 8, "totalDefense": 3, "shifts": [...], "finalOdds": "3:1", "possibleResults": [...] } }
```

```json
{ "type": "resolveBattle", "hexId": "2415", "unitIds": ["soviet.2gta.21motorRifleDivision"], "supportingHqId": "soviet.northernEastGermanyFront.hq" }
{ "type": "advanceAfterCombat", "unitIds": ["soviet.2gta.21motorRifleDivision"] }
```

Protocol 12 adds the snapshot's `combat` (battles, attacked units and hexes, Engaged units, pending advance) and the events `battleResolved`, `unitRetreated`, `advanceOffered`, and `unitsAdvanced`. Protocol 13 adds Offensive Support: `supportHqIds` on combat objectives, `supportingHqId` on battle reports, `combat.supportingHqIds`, and the `unitWithdrawn` event. Protocol 14 replaces the single `resupplyTargetUnitId` with `resupplyTargetUnitIds` and adds `selected` to the resupply command and event. Protocol 15 adds reserves: `battlePlan.reserveUnitIds`, the snapshot's `reserve`, the `setReserve` command, and the `reserveStatusChanged` and `reserveMarkersRemoved` events. Protocol 16 adds the `unitSupplyChanged { unitId, supply }` event and the map city flag `enclave`. Protocol 17 adds the movement modes `paradrop` and `seaTransport`, `battlePlan.sealiftStepsUsed`, and the map's `reinforcementSectors` (`{ number, sideId, hexId }`).

Reserve query and command:

```text
reserve_options()  →  { "revision": 4, "units": [{ "unitId": "...", "unavailable": null },
  { "unitId": "...", "unavailable": { "code": "enemyZoneOfControl", "message": "..." } }] }
```

```json
{ "type": "setReserve", "unitId": "eastGermany.2gta.8motorRifleDivision", "selected": true }
```

The snapshot adds `airPoints`, `strikePlan`, `airInterdictionZones`, `breakthroughMarkers`, `eliminatedUnitIds`, and each unit's `disruption`. New events: `airPointsReset`, `airMissionPlanned`, `airMissionCancelled`, `airStrikeResolved` (die roll, modifier, result), `airInterdictionZonePlaced`, `airInterdictionZonesRemoved`, `unitDisruptionChanged`, `unitStepLost`, `unitEliminated`, `breakthroughMarkerPlaced`, and `breakthroughMarkersRemoved`. The snapshot's `cities` array gives `{ hexId, owner, controller, free }` for every city hex. `CityControlChanged` events accompany movements that take or liberate a city, and their undo; each `PlannedMovement` records its `cityControlChanges`.

Command variant names and their fields are both camelCase. Battle-planning commands:

```json
{ "type": "setResupplyTarget", "unitId": "soviet.2gta.21motorRifleDivision", "selected": true }
{ "type": "setAttackTarget", "hexId": "2415", "selected": true }
{ "type": "moveUnit", "unitId": "soviet.2gta.21motorRifleDivision", "destination": "2411", "mode": "tactical" }
{ "type": "undoUnitMovement", "unitId": "soviet.2gta.21motorRifleDivision" }
{ "type": "entrainUnit", "unitId": "soviet.2gta.21motorRifleDivision" }
{ "type": "detrainUnit", "unitId": "soviet.2gta.21motorRifleDivision" }
{ "type": "undoDetrainUnit", "unitId": "soviet.2gta.21motorRifleDivision" }
```

## Desktop UI

The app opens on a title screen (`src/menu/TitleScreen.tsx`) with New Game, Load Game, Settings, and Quit; arrow keys and Enter work as well as the mouse. Load Game and Settings are placeholders. Quit calls the `quit_app` Tauri command. New Game opens the scenario screen (`src/menu/ScenarioSelect.tsx`). It lists every scenario returned by `list_scenarios`, so newly registered scenarios appear without frontend changes. `src/menu/scenarioCatalog.ts` adds presentation only: it groups registered scenarios into families (Introductory: BALTAP; Campaigns: Strategic Surprise, Extended Buildup, and War of Nerves; Development: the rules prototype) with 1983/1988 year variants and a short blurb. A registered scenario missing from the catalog still appears under "Other". Selecting a family shows its blurb, a year selector, its turns, map, identifier, and sides; ↑ ↓ change the family and ← → the year. Start Game calls `new_game` with the chosen scenario. `src/Root.tsx` switches between the screens and mounts the game screen (`App`) with the returned bootstrap, keyed by game ID.

The right-hand Control Panel is the primary view. With no unit selected it shows Go to Hex, then the selected hex's terrain, command zone, city/port data, hexside features, and occupying units; during Battle Planning it also lists the active side's Strategic Reserve and offers a toggle that adds the selected enemy-occupied hex as an attack objective or removes it. Choosing a unit (from the list, or by clicking its counter once its hex is already selected) switches the panel to a unit detail view with a drawn counter, identity, strength values, supply, rail status, traits, and, for the planning side's own units, planning actions. Every action button becomes an Undo button once its order is in the plan; the frontend derives that state from the authoritative `battlePlan`, never from local bookkeeping. The first left-click on a hex, counters included, only selects the hex and shows the hex view; clicking a counter in the already-selected hex opens that unit. Left-clicking elsewhere returns the panel to the hex view.

Movement is ordered on the map. While the planning side's own unit is selected, the panel shows a movement-mode selector (Tactical, March, Rail, Air transport). After a unit has moved, the other systems are disabled. The frontend calls `movement_options` once for the selected unit and again after each accepted command, never per pointer move. Mode buttons are enabled and explained only from that response. The objective toggle is enabled only for hexes in `attack_target_options`. The hex view shows each city's controller and whether it is Free or Conquered. Hovering a listed destination draws a red arrow along the core-chosen route. Right-clicking it submits `moveUnit`. Hexes not in the list show no arrow and ignore right-clicks. The selected unit stays selected after a move so that it can continue or be undone.

During the Offensive Strike Phase the frontend fetches `air_strike_options` once per revision. The top bar shows both sides' Air Points (Tactical · Operational · bonus). The hex view gains an Air Missions section with:

- the hex's Airspace;
- enemy units to check as targets, each with its core-computed die modifier;
- Strike and Interdict buttons for Tactical and Operational points, enabled only where the core allows them;
- each committed mission with an Undo button, replaced by its dice result once resolved.

While missions are pending, the top-bar action reads "Resolve Air Strikes" (`resolveAirStrikes`), then "End Phase". The map outlines strikeable hexes (red where Tactical points may be used, grey otherwise) and draws crosshairs for committed strikes, a purple seven-hex area for Air Interdiction Zones (lighter while pending), and stars for Breakthrough Markers. Counters show a D or S badge when Disrupted or Suppressed. The Battle Plan pane lists air missions and their results. The Combat Log is built on the client from strike events (die roll, modifier, result, step losses, eliminations, Disruption, Breakthroughs, interdiction); it is presentation only.

During the Combat Phase the frontend fetches `combat_options` once per revision. The map outlines attackable hexes: red for mandatory WP objectives, orange for optional ones. Hexes already fought over get a small cross.

The hex view gains a red Attack button. It is enabled only for hexes listed in `combat_options` (so WP is limited to its marked objectives and Breakthrough hexes, while NATO may attack any eligible hex); otherwise it is grey with the reason as a tooltip (already attacked, another advance pending, or no unit able to attack). The view also notes mandatory objectives and shows the result of a battle already fought there.

Attack opens the Battle Planner dialog (`src/ui/BattlePlanner.tsx`), titled "Battle Planner at <hex>":

- left: the defending units, each with its `battle_preview` adjusted Defense Strength and modifiers, plus a Free City's organic defense;
- centre: an SVG mini-map of the objective hex (outlined in red) and its six neighbours, drawn like the main map (terrain, water, coast, city outlines, and rivers, clipped to the seven hexes), with every unit shown as its full counter laid out side by side; below it the odds (totals, column shifts, final odds, and the six possible results);
- right: eligible attackers with checkboxes (all checked by default) and adjusted Attack Strengths, then an Offensive Support checkbox for each HQ the core lists (unchecked by default, since each HQ supports once per phase);
- each listed unit is joined to its counter on the mini-map by a polyline measured from the rendered layout, ending at the counter's nearest edge; unticked attackers draw dimmed and dashed;
- Attack (`resolveBattle`) and Cancel buttons.

After the battle the dialog stays open with the die roll, result, and Counterattacks, and each unit's new status (eliminated, retreated, Disrupted). When the hex is cleared, the right column becomes the advance choice (Advance or Stay in place), and the dialog cannot be closed until that decision is made. It reopens automatically if a pending advance exists.

After a battle the objective hex is selected automatically. The top-bar action reads "N Marked Attacks Left" and is disabled while WP objectives remain. Battles appear in the Battle Plan pane. The Combat Log lists odds, rolls, Counterattacks, losses, retreats, advances, and captured cities. A client-side roster keeps eliminated units' names.

The top bar shows the turn progress: the acting side (or Joint), "Round X / N", and a chevron track of that actor's non-automatic steps from the bootstrap `turnSequence` (Plan, Strike, Combat, Reserve). Steps before `turn.stepIndex` are ticked, the current one is filled in the side's colour, and later ones are dim.

During Battle Planning the unit detail view adds a "Mark as Reserve" (NATO) or "Mark as OMG" (WP) toggle. It is enabled from `reserve_options` for the current revision and otherwise shows the core's reason. Marked units get a gold RES/OMG tab on their counter (map and sidebar), a Reserve row in the unit data, a tag in unit lists, and a line in the Battle Plan pane. In the Reserve Phase the hex view lists the units to move. Selecting one shows the same movement controls as planning (the core leaves only Tactical enabled), with right-click orders and Undo; its moves appear in the Battle Plan pane. Breakthrough Zones are shaded yellow on the map. Out-of-supply units carry a red OOS tab above their counter; the unit data shows HQ supply for HQs and Movement/Combat supply for other units, highlighted when out of supply, and the Combat Log reports supply changes. The Combat Log reports the markers removed at the end of the Reserve Phase, Recovery (Disrupted removed), and Post-Battle unsuppression.

Display options (camera zoom, fit, and map-layer toggles) live in a modal Settings dialog opened from the top-right Settings button. The bottom bar is collapsible and split into two read-only panes: Battle Plan, rendered live from `snapshot.battlePlan`, and Combat Log, which stays empty until combat resolution exists.

Each later Joint Reinforcement Phase is resolved inside the `endPhase` that ends NATO's turn (turn one's happens inside `new_game`). The returned event stream includes:

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
      "location": { "type": "hex", "hexId": "2806" },
      "supply": {
        "headquarters": null,
        "movement": "supplied",
        "combat": "supplied"
      }
    }
  ]
}
```

The actual event contains every unit arriving that turn; the example shows one. The complete snapshot returned with it also lists every unit currently in play under `units`.

After Joint Reinforcement, `preBattle` resolves automatically and emits:

```json
{
  "type": "preBattleSupplyChecked",
  "gameTurn": 1,
  "sideId": "warsawPact",
  "units": [
    {
      "unitId": "soviet.6thGuardsMotorRifleDivision",
      "supply": {
        "headquarters": null,
        "movement": "supplied",
        "combat": "supplied"
      }
    }
  ]
}
```

The automatic phase then ends and the acting side's `battlePlanning` phase becomes current.
