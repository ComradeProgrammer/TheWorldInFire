# Turn Sequence

The scenario determines the game length. BALTAP 1983 lasts seven complete turns. The rules prototype and the 1983/1988 versions of Strategic Surprise, Extended Buildup, and War of Nerves last fourteen complete turns.

Each game turn consists, in order, of a Joint Player Turn, a Warsaw Pact Player Turn, and a NATO Player Turn.

## Joint Player Turn

The Joint Player Turn consists of these phases, in order:

1. Joint Status Phase
2. Joint Reinforcement Phase

Both phases resolve automatically and require no player input. In the Joint Reinforcement Phase each side receives its Air Points for the turn, and scheduled reinforcements arrive. On the first turn this deploys the opening forces, so a new game begins with the Warsaw Pact's Battle Planning Phase.

The Warsaw Pact begins its player turn after the Joint Player Turn ends.

## Player turn

Each player turn proceeds through these phases in order:

1. Pre-Battle Phase
2. Battle Planning Phase
3. Offensive Strike Phase
4. Combat Phase
5. Reserve Phase
6. Post-Battle Phase

The phases must be completed in this order. A player must finish the current phase before beginning the next and cannot return to a completed phase during the same player turn.

The Pre-Battle Phase resolves automatically and requires no player input. On entering it, the game checks the supply of every unit belonging to the acting side (see [Supply](supply.md)). Newly arrived reinforcements and units in the Strategic Reserve are supplied. Play then advances automatically to the Battle Planning Phase.

Battle Planning combines the player actions formerly spread across Movement, Recovery, and Battle Planning. In any order, the acting side may select one resupply target, move units, order units to entrain or detrain, use rail or air transport, add or remove attack objectives, and place units in reserve (see [Reserves](reserve.md)). Movement follows the [movement rules](movement.md). An attack objective must be a hex that currently contains at least one enemy unit, or an enemy Free City (see [Cities](cities.md)). Sea transport is not part of the current implementation. Entraining consumes one complete friendly planning phase; the unit becomes Entrained at the beginning of its side's next planning phase and may then move by rail.

Until the Battle Planning Phase ends, any order given during it can be taken back: clear the resupply choice, remove an attack objective, undo a unit's most recent movement, cancel an entrainment order, or restore the Entrained status of a unit detrained this phase. A detrainment cannot be undone after the unit has made a non-rail move, or when restoring it would exceed the side's rail capacity.

When the Battle Planning Phase ends, the acting side's Disrupted markers are removed.

In the Offensive Strike Phase, the acting side commits and resolves air missions: Air Strikes against enemy units and Air Interdiction Zones (see [Air power](air-power.md)). In the Combat Phase, the acting side attacks enemy-held hexes with adjacent units (see [Combat](combat.md)); the Warsaw Pact must attack every objective it marked during Battle Planning.

In the Reserve Phase, the acting side's Reserve or OMG units move again at half their Movement Allowance (see [Reserves](reserve.md)). At the end of the Reserve Phase, the acting side's Reserve/OMG markers and Breakthrough Markers and the enemy's Air Interdiction Zones are removed. The Post-Battle Phase resolves automatically and removes the acting side's Suppressed markers.

NATO begins its player turn after the Warsaw Pact completes its entire player turn. The game turn ends after NATO completes its player turn, and the next game turn begins with the Joint Status Phase.

The game ends after NATO completes its Post-Battle Phase in the scenario's final turn.
