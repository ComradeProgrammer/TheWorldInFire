# OOAW English Documentation

This directory contains OOAW's English rules and development notes. All content must remain synchronized with the corresponding Chinese documentation under `docs/zh-CN/`.

## Player rules

- [Turn sequence](rules/turn-sequence.md)
- [Movement](rules/movement.md)
- [Cities](rules/cities.md)
- [Air power and the Offensive Strike Phase](rules/air-power.md)
- [Combat](rules/combat.md)
- [Joint reinforcements](rules/reinforcements.md)
- [BALTAP 1983](rules/baltap.md)
- [Campaign scenarios](rules/campaigns.md)

`rules/` is the player-facing rulebook. It contains only rules currently in effect and excludes code structure, APIs, events, revisions, development progress, and future plans.

## Development notes

- [Turn state machine development notes](develop/turn-state-machine.md)
- [Campaign data and implementation scope](develop/campaign-scenarios.md)

`develop/` is developer-facing and records implementation status, design decisions, IPC, unfinished work, and future plans.
