# Turn Sequence

The scenario determines the game length. BALTAP 1983 lasts seven complete turns. The rules prototype and the 1983/1988 versions of Strategic Surprise, Extended Buildup, and War of Nerves last fourteen complete turns.

Each game turn begins with the Joint Phases. The two sides then take turns phase by phase: the Warsaw Pact plays a phase, NATO plays the same phase, and only then does the turn move on to the next phase.

## Joint Phases

1. Joint Status Phase
2. Joint Reinforcement Phase

Both phases resolve automatically and require no player input. In the Joint Reinforcement Phase every surviving air unit becomes ready, and scheduled reinforcements arrive. On the first turn this deploys the opening forces.

## Side phases

After the Joint Phases, the turn proceeds through these phases in order, each played first by the Warsaw Pact and then by NATO:

| Phase | Warsaw Pact | then NATO |
| --- | --- | --- |
| 1. Pre-Battle Phase | automatic | automatic |
| 2. Battle Planning Phase | plays | plays |
| 3. Joint Air Operations Phase | automatic fighter combat and interception for both sides | — |
| 4. Offensive Strike Phase | automatic resolution and review | automatic resolution and review |
| 5. Combat Phase | plays | plays |
| 6. Reserve Phase | plays | plays |
| 7. Post-Battle Phase | automatic | automatic |

A new game therefore begins with the Warsaw Pact's Battle Planning Phase. A side must finish its current phase before play passes on, and cannot return to a phase it has completed. Everything a side decides in its Battle Planning Phase (attack objectives and Reserve/OMG units) stays in force through its own later phases, while the other side plays its phases in between.

The Pre-Battle Phase resolves automatically. On entering it, the game checks the supply of every unit belonging to the acting side (see [Supply](supply.md)). Newly arrived reinforcements and units in the Strategic Reserve are supplied.

Battle Planning combines the player actions formerly spread across Movement, Recovery, and Battle Planning. In any order, the acting side may select one resupply target, move units, order units to entrain or detrain, use rail or air transport, add or remove attack objectives, place units in reserve (see [Reserves](reserve.md)), and assign missions to available air units (see [Air Power](air-power.md)). Movement follows the [movement rules](movement.md). An attack objective must be a hex that currently contains at least one enemy unit, or an enemy Free City (see [Cities](cities.md)). Sea transport is not part of the current implementation. Entraining consumes one complete friendly planning phase; the unit becomes Entrained at the beginning of its side's next planning phase and may then move by rail.

Until the Battle Planning Phase ends, any order given during it can be taken back: clear the resupply choice, remove an attack objective, undo a unit's most recent movement, cancel an entrainment order, or restore the Entrained status of a unit detrained this phase. A detrainment cannot be undone after the unit has made a non-rail move, or when restoring it would exceed the side's rail capacity.

When the Battle Planning Phase ends, the acting side's Disrupted markers are removed. Because the enemy's Offensive Strike and Combat Phases now come before your own next Battle Planning, units the enemy disrupts stay Disrupted through your own Combat Phase in the same turn.

After both plans are complete, Joint Air Operations automatically resolve fighter combat and fighter interception of strike aircraft. At the start of each Offensive Strike Phase, the acting side's surviving, unaborted fighter-bombers automatically perform their planned ground or airbase strikes; the player reviews those results and ends the phase (see [Air Power](air-power.md)). In the Combat Phase, the acting side attacks enemy-held hexes with adjacent units (see [Combat](combat.md)); the Warsaw Pact must attack every objective it marked during its Battle Planning.

In the Reserve Phase, the acting side's Reserve or OMG units move again at half their Movement Allowance (see [Reserves](reserve.md)). At the end of the Reserve Phase, the acting side's Reserve/OMG markers and its own Breakthrough Markers are removed, together with the enemy's Air Interdiction Zones. The Post-Battle Phase resolves automatically and removes the acting side's Suppressed markers.

The game turn ends after NATO's Post-Battle Phase, and the next game turn begins with the Joint Status Phase. The game ends after NATO's Post-Battle Phase in the scenario's final turn.
