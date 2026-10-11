# Year variants of the three campaign scenarios

The backend registers the following six options. `list_scenarios()` and `new_game(scenario_id)` share the registry. The frontend reads the list and passes back the ID without maintaining its own titles or deployment tables. BALTAP 1983 and the rules prototype remain available, for eight registered entries.

| ID | Units on war turn one | Total units within fourteen turns |
| --- | ---: | ---: |
| `nato-strategic-surprise-1983` | 199 | 287 |
| `nato-strategic-surprise-1988` | 202 | 286 |
| `nato-extended-buildup-1983` | 341 | 362 |
| `nato-extended-buildup-1988` | 339 | 360 |
| `nato-war-of-nerves-1983` | 199 | 287 |
| `nato-war-of-nerves-1988` | 202 | 286 |

## Data and implemented behavior

`crates/plugins/nato-official/data/natoCampaigns.json` contains structured data embedded at build time. Attack, defense, movement, strength steps, type, nationality, and affiliation come from the order of battle in section 44 of the living Play Booklet updated 1 January 2026. Opening positions and arrivals come from the six corresponding standard VASSAL 2.4.1 setups, excluding Alternate NATO variants. West German 9/3Pz starts at 2716 per the living erratum in 37.3/40.3.

`tools/import_nato_campaigns.py` regenerates the data from local `internet/` references using Python and pypdf. It reads inputs without modifying them; runtime, builds, and tests do not depend on `internet/`. Soviet 6G and 90GT exchange Fronts in 1988 per 44.2. British upgrades, Canadian reinforcements, and the U.S. 81st Brigade's year changes use the actual year-specific data. The 1988 Soviet 83rd Air Assault Brigade from the scenario text and module is included as well.

All six options use the existing fourteen-war-turn sequence and scheduled reinforcements. Strategic Surprise and War of Nerves have three WP and two NATO Airlift Commands; Extended Buildup has four WP and three NATO commands. Strategic Surprise has Surprise on turn one. War of Nerves does not assume Surprise while Alert Levels are unmodeled. Eligible Front HQs and U.S. V/VII Corps HQs provide Offensive Support; Baltic Front and U.S. III Corps HQs do so only in Extended Buildup.

**Fortified hexes (38.4.3.4).** `natoCampaigns.json` lists `fortifiedHexes`, and only Extended Buildup copies it into `ScenarioDefinition::fortified_hexes`. The list has 44 hexes: the 43 West German hexes adjacent to an Iron Curtain hexside, plus West Berlin (3007). The map data has no Iron Curtain hexsides; the traced `ironCurtain` polylines cover only part of the border. The 85 Iron Curtain hexsides were therefore read from the printed map image, using the line's orange casing along each hexside, and the Elbe stretch was taken from the `majorRiver` hexsides between the orange sections. They were checked to form one unbranched line from Lübeck Bay to the southern map edge, and the West German hex of each was taken on the west side of that line. `Rules::new` copies the list into rules state as `fortifiedHexIds`, which the snapshot gives the frontend. By house rule the list never changes: unlike 38.4.3.4, WP occupation at a Joint Status Phase does not Conquer a Fortified hex. Effects apply only to WP attacks and Strikes against NATO:
- the Objective's terrain column shift is replaced by `ColumnShiftReason::Fortified` (−2);
- NATO Soft defenders double with `StrengthModifier::FortifiedHex` in place of `SoftUnitCover`;
- retreats are reduced by `max(1, terrain reduction)`, and a NATO unit may stop after one hex when its first retreat hex is Fortified;
- the Strike terrain modifier is replaced by −2, unless the target is under a train marker.

Exploitation is not implemented, so the no-Exploitation rule needs no code. War of Nerves' Alert Level Six fortification (39.4.6, 40.4.3.5) depends on the unimplemented Alert Level system. The Nuclear Strike negation (21.1.3) is likewise not implemented.

## Differences from the complete board-game scenarios

These are campaign entries for the current war-turn engine, not complete implementations of the board-game scenarios:

- Strategic Surprise's special pre-war GT0, Movement to GDP, and progressive activation are not implemented; play starts on war turn one.
- War of Nerves' Peace Turns, Tension, Alert Levels, Preparation Measures, and mobilization clocks are not implemented. It currently uses fixed war-turn reinforcements, so its active unit schedule matches Strategic Surprise in the same year. Mobilization arrivals beyond fourteen turns remain in `deferredUnits` and are not brought forward into the current game.
- Reforger Sites, arriving Reforger Steps, and conversion are not implemented. By house rule Reforger units are ground reinforcements: outside Extended Buildup they stand on their Reforger site hex from the start.
- Reinforcement Boxes are replaced by map-edge entry hexes (house rule). The import tool maps each later arrival's Entry Code from the order of battle: `G#` to sector #'s entry hex; `RS`/`RF` to the printed site hex; `RR` to a sector chosen by formation (Czechoslovak and Carpathian fronts 3; Belorussian front, 5th Guards Tank Army, and the GSFG fronts 4; Polish and Baltic fronts 5; British forces 1; French 2) with `"entrained": true`; `A`, `S`, and `EB` arrivals to the Strategic Reserve (`hex: null`), from which they leave by air or sea.
- Extended Buildup retains the module's assembly positions, with some formations concentrated in a single assembly hex. Its free deployment areas and setup phase are not implemented. These initial stacks may exceed ordinary end-of-movement limits; destination stacking validation still applies.
- Optional Extended Buildup XVIII Corps intervention units remain in `deferredUnits`; no arrival turn is chosen on the player's behalf.
- Air Campaign Tables still use the existing prototype Air Point allocations. Maximum Effort, nuclear/chemical resources, complete Automatic Supply, sealift, helicopters, and scenario victory conditions are not implemented. The game ends after fourteen war turns without adjudicating the original victory table.

`campaign_scenarios.rs` lists and creates all six versions through the JSON process interface, checks year-specific strengths and affiliations, errata, airlift and support configuration, and runs all fourteen turns to verify opening deployment, later arrivals, and completion.
