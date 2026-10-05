# Air Power

Named squadron counters represent air forces. Every counter has full and reduced steps and belongs to an off-map airbase. The current system has three air-unit types:

- **Fighters** fly air-superiority missions, engage enemy fighters within their combat radius, and intercept enemy fighter-bombers.
- **Fighter-bombers** strike ground units or enemy off-map airbases.
- **Airborne early-warning aircraft (AEW)** modify friendly air combat within their support radius. AEW aircraft cannot be attacked in the current version.

An aircraft that takes one step loss flips to its reduced side; a further step loss eliminates it. An aborted aircraft cannot continue its mission that turn. During each new turn's Joint Reinforcement Phase, every surviving aircraft becomes ready again. Lost steps do not recover automatically.

## Off-map airbases

Every aircraft has a fixed home airbase. Each base has a map-edge anchor hex and a sortie capacity. The anchor represents the approach route for interception and display; the current version imposes no base-to-target range limit.

Airbases can be struck:

- 0 damage: full sortie capacity;
- 1 damage: half capacity, rounded up;
- 2 damage: closed and unable to launch sorties;
- suppressed: unable to launch sorties while suppression lasts.

Damage does not affect aircraft that already launched that turn. Airbase damage does not repair automatically in the first version.

## Battle Planning Phase

During its Battle Planning Phase, a side may assign at most one mission to each available aircraft. A mission may be withdrawn before that phase ends.

| Aircraft | Mission | Selection |
| --- | --- | --- |
| Fighter | Air superiority | a combat-area center hex |
| Fighter-bomber | Ground strike | a target hex and up to two steps in that hex |
| Fighter-bomber | Airbase strike | one enemy off-map airbase |
| AEW | Early warning | a support-area center hex |

Each aircraft flies at most once per turn, and all sorties count against the effective capacity of their home airbase. A ground strike cannot name the same unit twice or name a unit already targeted by another air strike that turn. An HQ must be targeted alone.

## Joint Air Operations Phase

After both sides finish Battle Planning, the game automatically resolves Joint Air Operations. Every roll and modifier is preserved in the event log and air-operations report.

### Fighter combat

A fighter projects a combat area from its mission center, using the combat radius on its current step. Two hostile fighters may engage when their combat areas intersect. The game uses deterministic maximum matching so that as many aircraft as possible engage, with each aircraft fighting at most once per round.

Both aircraft attack simultaneously. For each attack, subtract the defender's evasion from the attacker's air-combat value to find the table column, limited to −4 through +4. Roll a d20, apply an eligible AEW modifier, and consult the Air Combat Table. Apply both results together. Fighters that can still operate fight another round while hostile combat areas continue to intersect.

An AEW counter provides the modifier printed on its current step to combat within its support radius. Multiple AEW modifiers do not add; use the highest eligible modifier.

### Fighter interception

After fighter combat, each fighter still able to operate checks the target hexes of enemy fighter-bombers. It may intercept a mission whose target lies within its combat radius. Each fighter may intercept one fighter-bomber, and each fighter-bomber may be intercepted once.

Interception uses the same simultaneous Air Combat Table procedure, so the fighter-bomber fires back. An aborted or eliminated fighter-bomber does not perform its planned strike. A fighter-bomber that loses one step is also aborted by the corresponding table result.

### 1985 Air Combat Table

Results are: `—` no effect, `A` abort, `DA` one step loss and abort, `1A` destroyed/one step loss and abort, and `1DA` destroyed plus a damaged step and abort. With the current two-step counters, `DA` and `1A` each remove one step, while `1DA` eliminates a full-strength counter.

| Column | — | A | DA | 1A | 1DA |
| ---: | :---: | :---: | :---: | :---: | :---: |
| −4 | 1–15 | 16–18 | 19–20 | — | — |
| −3 | 1–13 | 14–16 | 17–19 | 20 | — |
| −2 | 1–11 | 12–14 | 15–17 | 18–20 | — |
| −1 | 1–10 | 11–13 | 14–16 | 17–19 | 20 |
| 0 | 1–9 | 10–12 | 13–15 | 16–18 | 19–20 |
| +1 | 1–8 | 9–11 | 12–14 | 15–17 | 18–20 |
| +2 | 1–7 | 8–10 | 11–13 | 14–16 | 17–20 |
| +3 | 1–5 | 6–8 | 9–11 | 12–14 | 15–20 |
| +4 | 1–3 | 4–6 | 7–9 | 10–12 | 13–20 |

## Offensive Strike Phase

After Joint Air Operations, each surviving, unaborted fighter-bomber automatically performs its planned mission at the start of its side's Offensive Strike Phase. The phase remains an interactive review stop so the player can inspect the results before ending it.

### Ground strikes

Ground strikes use the existing NATO strike procedure. Roll a d6 for each fighter-bomber and add its current-step strike modifier and the target modifiers:

| Condition | Modifier |
| --- | ---: |
| Target in a Major or Key City | −2 |
| Target in a Forest, Rough, Mountain, or Minor City hex | −1 |
| Target under a train marker, replacing terrain | +1 |
| Target in the striking side's friendly ground Airspace | +1 |
| Target in the striking side's enemy ground Airspace | −1 |
| Warsaw Pact strike during NATO's Surprise turn | +1 |

| Modified roll | Result |
| --- | --- |
| 1 or less | No effect |
| 2–4 | Disrupt every target |
| 5 or more | First target loses one step; disrupt the remaining targets |

If every named target has disappeared before resolution, record `targetGone`; do not reroll or select a new target.

### Airbase strikes

Roll a d6 and add the aircraft's strike modifier and the target airbase's modifier:

| Modified roll | Result |
| --- | --- |
| 1 or less | No effect |
| 2–4 | Suppress the base through the end of the next turn |
| 5 or more | Add one permanent damage and suppress the base through the end of the next turn |

## Ground Airspace

The original static Airspace rules remain in force for ground movement, rail movement, and air transport. Supplied ground units and controlled cities project that Airspace. Fighter combat areas are a separate concept used only for air combat and interception, so they do not retroactively invalidate ground movement already completed this turn.
