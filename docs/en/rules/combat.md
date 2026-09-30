# Combat

In the Combat Phase, the acting side attacks enemy-held hexes with adjacent Maneuver units. Each attack targets one **Objective hex** and is resolved as one battle. The attacker makes every decision; the defender's responses follow the fixed rules below.

## Choosing an Objective

An Objective hex must contain an enemy unit or an enemy Free City, or be an empty hex holding a friendly Breakthrough Marker.

- The **Warsaw Pact** may attack only hexes it marked as attack objectives during Battle Planning, plus Breakthrough Marker hexes. It must attack every marked objective that it still can before the phase may end.
- **NATO** may attack any eligible hex; its marked objectives are optional.
- No hex may be attacked twice in the same phase, and no unit may attack twice.

## Committing units

Any friendly Maneuver unit adjacent to the Objective hex may attack, except:

- HQs, which never attack;
- units under a train marker;
- units in reserve (see [Reserves](reserve.md));
- units that would attack across a Blocked or All-Sea hexside or the Danish Ferry.

Units with an Attack Strength of 0 may join an attack but never attack alone.

## Offensive Support

When committing an attack, the attacker may add **Offensive Support** from one eligible HQ for one column shift upward. The scenario lists which HQs can give support and which formations each commands. The HQ:

- must be in supply, not Suppressed, and not under a train marker;
- may support only one battle per Combat Phase;
- must reach at least one committed unit of its own formations within its Support Range. The path may not pass through enemy units, enemy-controlled cities, enemy-ZOC hexes without a friendly unit, or all-sea hexes or hexsides, and may not cross Blocked hexsides (the Danish Ferry is allowed). Count the HQ's hex but not the unit's.

A battle may receive support from only one HQ. The HQ itself does not take part in the battle and is never affected by its results.

## Odds

Add up the attackers' strengths, keeping fractions, and round down. Each attacking unit's Attack Strength is:

- halved if Disrupted;
- halved if Out of Combat Supply;
- halved for Armored units attacking into a City or Mountain hex;
- reduced by a quarter across a Minor River hexside, or halved across a Major River hexside.

Add up the defenders' strengths and round up (never below 1). Each defending unit's Defense Strength is:

- halved if Disrupted;
- halved if Out of Combat Supply;
- doubled for Soft units in Forest, Rough, Mountain, or City hexes.

An enemy Free City adds its Organic Defense Strength. An HQ adds its Defense Strength only if no Maneuver unit defends the hex. Units under a train marker, and units that already retreated from an earlier battle this phase, add nothing.

Divide attack by defense and round in the defender's favour to find the odds column, from 1:4 to 10:1. Then shift the column:

| Condition | Shift |
| --- | ---: |
| Objective in a Major or Key City | −2 |
| Objective in a Forest, Rough, Mountain, or Minor City hex | −1 |
| Flank Attack: every adjacent hex holds an attacking unit or is in its ZOC, and the defenders are next to another friendly unit or Free City | +1 |
| Concentric Attack: surrounded as above, with no such neighbour | +2 |
| Warsaw Pact attack on the turn NATO is Surprised | +1 |
| Offensive Support from an HQ | +1 |

The net shift is limited to two columns either way, except that a Warsaw Pact attack on the Surprise turn has no upward limit.

## Combat Results Table

Roll one die on the final column. Results read *attacker / defender*:

- **A1:** the attacker loses one step.
- **\*:** that side's units are Disrupted.
- **CA:** each defending step of an undisrupted, supplied Maneuver unit Counterattacks: one die against the strongest attacker. It Disrupts the target on 3+ (West German), 4+ (other NATO or Soviet), or 5+ (other Warsaw Pact). NATO Counterattacks with one nationality only: the one with the most eligible steps.
- **D#:** the defender loses that many steps. Maneuver units lose them first, strongest first. An HQ is Suppressed instead of reduced. Units under a train marker that defend alone are eliminated.
- **R#:** the defenders retreat that many hexes.

Results apply in this order: Counterattacks, defender losses, retreat, defender Disruption, attacker losses, attacker Disruption. The attacker ignores an A1 if the defender could not absorb all of its own losses.

## Retreat

Rough terrain or a Minor City reduces a retreat by one hex. Mountain or a Major or Key City reduces it by two.

The defenders retreat together and must end that many hexes from the Objective hex. They may not enter an enemy unit, an enemy-controlled city, or Prohibited Terrain. Among the legal routes, they prefer:

1. the fewest step losses;
2. not ending next to an enemy unit;
3. not overstacking;
4. avoiding Mountains and Major River crossings;
5. ending as far from the attackers as possible.

A unit loses a step for each enemy-ZOC hex it enters without a friendly unit or Free City there, and for each hex it cannot retreat. If its first hex holds a friendly unit or retreat-reducing terrain, it may stop there. Units under a train marker that must retreat are eliminated.

Surviving defenders cannot defend again this phase.

## Advance

When no defenders remain, the attacker may advance any surviving attacking units into the Objective hex, up to the stacking limit, or choose not to advance. Then a Breakthrough Marker is placed in the hex.

- An enemy **Free City** defending alone loses its Organic Defense only when the battle inflicts a step loss. Otherwise the attacker cannot advance.
- Advancing into an enemy city takes control of it.
- While an enemy Free City still holds the hex (because no unit advanced), no Breakthrough Marker is placed.

Attacking an empty Breakthrough Marker hex needs no die roll: the committed units may simply advance.
