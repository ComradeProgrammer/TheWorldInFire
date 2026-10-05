# Air Unit System Rewrite Plan

> Status: the first backend version is implemented and ready for frontend integration. Empty legacy Air Point fields remain temporarily for frontend migration compatibility.

## Scope

The current system spends refreshed `AirPoints` on strikes and interdiction. The replacement represents air power with named, based, two-step counters inspired by 1985.

The first version has three unit kinds:

- `fighter`: sorties to a map hex, projects a combat radius, fights enemy fighters, and intercepts strike aircraft;
- `fighterBomber`: sorties against a ground hex or enemy off-map airbase, resolves the existing NATO strike procedure, and may defend itself when intercepted;
- `aew`: sorties to a map hex and modifies friendly air combat in its support radius; it cannot be attacked in the first version.

ECM, SEAD, Flak, transport-aircraft counters, aircraft replacements, airbase repair, and the existing air-interdiction mission are deferred. Existing NATO Airlift Commands and transport movement remain independent because the requested roster contains no transport-aircraft type.

## Architectural decision

Air units must be plugin-owned state rather than generic kernel `Unit` values. Generic units are assumed to participate in ground supply, stacking, zones of control, city capture, and map movement, and `UnitLocation` has only map-hex and strategic-reserve variants. The kernel already stores arbitrary plugin-owned rule state, so this design requires no `ooaw-core` or generic plugin-API change.

The current static NATO `Airspace`, projected by supplied ground units and controlled cities, remains in place for march, rail, air transport, sea transport, and the original NATO modifiers. New fighter coverage is a separate concept used only for air combat and interception. Replacing static Airspace during planning would otherwise retroactively invalidate ground moves made before the joint air battle is resolved.

## Domain model

Add plugin-local `AirUnitId` and `AirBaseId` newtypes. Do not reuse `UnitId`.

An `AirUnitDefinition` contains its ID, display name, side, nation, `AirUnitKind`, home `AirBaseId`, and exactly two `AirUnitStep` entries. Step data contains `airCombat`, `evasion`, `strikeModifier`, `combatRadius`, `aewModifier`, and `aewRadius`; scenario validation rejects invalid combinations. `AirUnitState` adds `strengthStepIndex` and `AirReadiness` (`ready`, `aborted`, or `flown`). Eliminated counters leave active state and enter `eliminatedAirUnitIds`.

Names follow the 1985 counter convention, for example `US 525th TFS — F-15C`, with a stable ID such as `us.525.f15c`. Names and values are normalized into tracked scenario JSON. Runtime and CI must not depend on `internet/`, and original counter artwork is not copied.

An off-map `AirBaseDefinition` contains an ID, name, side, `anchorHexId`, sortie capacity, and strike modifier. The anchor is a map-edge approach point used for interception, mission-line rendering, and future range rules. The first version imposes no base-to-target flight range.

`AirBaseState` adds zero-to-two damage and an optional `suppressedThroughTurn`. Capacity is full at zero damage, rounded-up half at one damage, and zero at two damage. A suppressed base has zero capacity. There is no automatic repair in the first version, and a strike against a base does not affect aircraft that already launched that turn.

Air missions are tagged variants:

```text
airSuperiority { centerHexId }
groundStrike { hexId, unitIds }
airBaseStrike { airBaseId }
earlyWarning { centerHexId }
```

Each side has one `AirPlan` per game turn containing stable `AirSortie` IDs. Rules state gains `airUnits`, `airBases`, `airPlans`, `airOperationsReport`, and `eliminatedAirUnitIds`. The report retains pairings, natural rolls, modifiers, table columns, raw results, aborts, and losses for UI animation, saves, and replay.

## Turn sequence

Refactor the generated sequence into an explicit sequence:

```text
Joint Status
Joint Reinforcement (ready aircraft)
WP Pre-Battle
NATO Pre-Battle
WP Battle Planning (ground orders and air sorties)
NATO Battle Planning (ground orders and air sorties)
Joint Air Operations (automatic fighter combat and interception)
WP Offensive Strike (automatic resolution on entry; interactive review stop)
NATO Offensive Strike (automatic resolution on entry; interactive review stop)
WP/NATO Combat
WP/NATO Reserve
WP/NATO Post-Battle
```

This defines “resolve air combat when Strike begins” as one joint automatic phase after both plans are locked and before the first Offensive Strike. Strike targets are chosen during planning. Missions resolve automatically when each Offensive Strike step begins, while the interactive phase remains as a result-review stop. WP retains the current first-strike ordering. A later mission whose named targets have all disappeared records `targetGone` and neither rerolls nor retargets.

## Planning contract

Replace the old strike commands with:

```text
planAirSortie { airUnitId, mission }
cancelAirSortie { sortieId }
```

Validation checks the acting Battle Planning side, ownership, surviving/ready state, one sortie per aircraft, effective base capacity, mission/type compatibility, target existence, the existing two-ground-step limit, the HQ-alone restriction, and enemy ownership for airbase targets.

Add queries:

```text
airPlanningOptions
airMissionOptions { airUnitId }
fighterCoverage { sideId }
```

Plans may remain visible in the shared snapshot for the current local game. A future multiplayer view layer should hide unresolved enemy plans without changing authoritative resolution.

## Fighter combat

A fighter covers every hex whose hex distance from its mission center is at most its current-step `combatRadius`. Two hostile fighter zones intersect when the distance between their centers is at most the sum of their radii.

Each combat round builds a bipartite graph from all active hostile intersections and computes a deterministic maximum matching. Shorter center distance wins ties, followed by stable air-unit IDs. Every matched pair fires simultaneously, results are then applied together, aborted and eliminated aircraft leave the operation, and the graph is rebuilt. Rounds continue until there is no hostile intersection. A 20-round guard records disengagement if invalid table data would otherwise prevent termination.

This matching already models numerical advantage, so the first version does not also apply the 1985 numerical-superiority modifier.

Use the 1985 d20 Air Combat Table:

```text
column = clamp(attacker.airCombat - defender.evasion, -4, +4)
modifiedRoll = d20 + AEW modifier
```

Only AEW modifies the first version. Pilot, ECM, and separate numerical-superiority modifiers are deferred. Raw table results map to the two-step counters as follows:

| Raw result | State effect |
| --- | --- |
| no effect | remains on station |
| `0-0-a` | aborts with no step loss |
| `0-1-a` | loses one step and aborts |
| `1-0-a` | loses one step and aborts |
| `1-1-a` | loses two steps and aborts; a full counter is eliminated |

The report preserves the distinction between damaged and destroyed raw steps for a future repair system.

AEW supports a fighter when its mission center lies within the AEW radius. During interception it supports a fighter at the fighter center and a fighter-bomber at the strike hex or target airbase anchor. Only the highest friendly AEW modifier applies. Initial data should use the 1985 values of NATO `+2` and WP `+1`, stored on unit steps rather than hard-coded by side.

## Interception

After fighter combat, every surviving, non-aborted fighter may intercept one enemy fighter-bomber whose target lies in its coverage. Each fighter-bomber may be intercepted once. Resolve a deterministic maximum matching by shortest distance and stable IDs.

An interception has one simultaneous air-combat round. The fighter-bomber fires back with its own values. An aborted or eliminated fighter-bomber loses its strike; a fighter-bomber that is neither aborted nor eliminated proceeds even if its counter was reduced. A fighter that survived the fighter battle may still perform one interception.

## Strike resolution

Surviving fighter-bombers reuse the current NATO d6 Strike Table, terrain/city and train modifiers, Surprise, ground-unit disruption and step loss, and Breakthrough Markers. Add the aircraft step's `strikeModifier`; initial data may set it to zero. Retain the current static-Airspace `+1/0/-1` modifier in the first version to avoid combining this rewrite with a ground-movement rebalance.

Off-map airbases use the same table:

| Result | Airbase effect |
| --- | --- |
| `NoEffect` | none |
| `Disrupted` | suppressed through the next game turn |
| `StepLoss` | add one persistent damage and suppress through the next turn; two damage closes the base |

The airbase's strike modifier also applies. Based aircraft are not directly damaged.

## Protocol events

Remove or disable `planAirStrike`, `planAirInterdiction`, `cancelAirMission`, and `resolveAirStrikes`. Add these authoritative events:

```text
airUnitsReadied
airSortiePlanned
airSortieCancelled
airCombatRoundResolved
airUnitStepLost
airUnitAborted
airUnitEliminated
airInterceptionResolved
airStrikeResolved
airStrikeAborted
airBaseSuppressed
airBaseDamaged
```

Every random event carries the natural roll, modifier, modified roll, table column, and raw result. The frontend displays and animates events but does not reproduce rule calculations.

## Code layout and migration

Add:

```text
crates/plugins/nato-official/src/model/air.rs
crates/plugins/nato-official/src/air_operations.rs
crates/plugins/nato-official/src/air_strikes.rs
crates/plugins/nato-official/data/natoAirForces.json
crates/plugins/nato-official/tests/air_operations.rs
```

Modify the model exports, scenario builders, rules state, phase hooks, command/event enums, plugin manifest, debug setup, and `src/gameApi.ts`. Before removing the old `strikes.rs`, move its generic ground disruption, step-loss, and recovery helpers into `casualties.rs` or the appropriate ground modules.

Implement in five passing stages:

1. Add serializable state and a small test-scenario roster while the point system still compiles.
2. Add planning commands/queries and insert an initially no-op `jointAirOperations` phase.
3. Implement d20 fighter combat, stable matching, simultaneous loss, abort, and AEW.
4. Implement interception, automatic ground strikes, and airbase strikes.
5. Populate the six 1983/1988 campaign variants, add an explicit BALTAP/prototype policy, remove the point system, and update both player rulebooks.

## Required tests

Tests must cover radius boundaries, stable many-to-many matching, simultaneous mutual destruction, flip/elimination/abort mapping, AEW range and non-stacking, one interception per fighter and fighter-bomber, aborted strikes, all retained NATO ground-strike rules, airbase capacity and next-turn effects, already-launched aircraft surviving base damage, deterministic replay and rollback, and regressions for static Airspace, march, rail, air/sea transport, and ground combat.

The first version is complete when a player can assign one legal mission to each available named aircraft during Battle Planning; the engine automatically and reproducibly resolves fighter combat, AEW and interception; surviving fighter-bombers automatically strike ground units or off-map bases; and every step loss, abort, base state, and mission outcome appears in the authoritative snapshot and event stream.
