# Map rendering decision

Date: 2026-09-23

## Decision

- **Renderer:** the map is drawn with PixiJS 8 (`pixi.js`), inside a React host component (`src/map/MapCanvas.tsx`).
- **No map images:** the map is not a bitmap. Everything is drawn from vector data in `crates/ooaw-core/data/natoMap.json`, which was traced once from the reference VASSAL boards by a local, uncommitted extraction script.
- **Authoritative ownership:** the map is part of each Rust `ScenarioDefinition`. The `new_game` bootstrap sends the selected map to the frontend together with the initial game snapshot. Later command snapshots carry only its stable `mapId` and do not resend the full map.

## Coordinates

- **Map pixels:** world coordinates are the stitched VASSAL board space, 5651 × 7243 px.
- **Hex centres:** `x = 15 + 133.5·(38 − col) + (row odd ? 66.75 : 0)`, `y = −3 + 114.4·row`.
- **Hex ids:** the printed `RRCC` numbers are the stable hex ids. Commands to the Rust core should use these ids, not screen positions.

`src/map/hexGrid.ts` implements client-side rendering geometry. The Rust core owns the same grid definition and stable hex IDs through its scenario map.

## Data ownership

- **Rules input:** `natoMap.json` is authoritative terrain reference data embedded in `ooaw-core`. The rules can directly use:
  - `hexes[].terrain`
  - `city`
  - `port`
  - `coastal`
  - `commandZone`
  - `hexsides`

  Scenario validation already checks that deployed units and hexside endpoints reference hexes present in this map.
- **Presentation only:** `water`, `lines`, `cities` outlines, `symbols` and `labels`.
- **Primary terrain:** `terrain` is the natural terrain. A city in the hex takes priority (TEC priority 1–2). That priority is resolved by the rules, not in the data.

## Renderer structure

| Module | Responsibility |
| --- | --- |
| `render/terrainLayer.ts` | Procedural terrain art (seeded per hex, deterministic) |
| `render/featureLayers.ts` | Water, coast, rivers, grid, borders, hexside features, command zones |
| `render/symbolLayers.ts` | Cities, defense boxes, ports, mobilization sites, labels, hex numbers |
| `render/unitLayer.ts` | Programmatically drawn unit counters and stack badges from authoritative unit state; returns counter footprints for click hit testing |
| `render/camera.ts` | Pan and zoom (local view state only) |
| `render/MapRenderer.ts` | Application lifecycle, layers, level of detail, hit testing, hover and selection |

Hover, selection and camera are frontend-only state. No IPC happens during pointer movement. Counter clicks are hit-tested inside `MapRenderer`'s own pointer handler (not Pixi per-object events), so a map drag that starts on a counter never selects a unit. Right-click (button 2 without dragging) is reserved for move orders: `setMovementPreview` supplies the core's legal destinations for the selected unit. Hovering one of them draws a red route arrow on the `move-preview` layer, and right-clicking it calls `onMoveOrder`. Other hexes show nothing and ignore right-clicks. The browser context menu is suppressed on the canvas.

The frontend contains no compiled-in copy of the NATO map. `App.tsx` creates the BALTAP game through Tauri, waits for the bootstrap response, and only then constructs `MapCanvas`, `HexGrid`, and `MapRenderer` from the returned map.

## Unit counters

Unit counters are generated at runtime with PixiJS primitives and text. NATO and Warsaw Pact counters use the application's own palette, while simple geometric symbols distinguish headquarters, armor, mechanized, airborne, marine, territorial, and other formations. Combat values come from the active strength step in the Rust snapshot. Multiple units in one hex are offset as a stack and receive a numeric stack badge. No scanned or extracted counter image from the reference game is loaded by the frontend.

When the player ends the opening Joint Status Phase, the frontend submits `EndPhase` to the Rust core. It consumes the returned `reinforcementsArrived` event and updated snapshot, redraws the unit layer, and focuses the camera on the deployed units. `setBattlePlan` draws the active plan's movement paths and attack-objective outlines. `setStrikeOverlay` draws Offensive Strike state from the core. Air Interdiction Zones (the marked hex and its six neighbours) go on the `air-interdiction` layer beneath the counters. Strikeable-hex outlines, strike crosshairs, and Breakthrough stars go on the `air-strikes` layer above them. Counters carry an orange D or S badge while Disrupted or Suppressed. `setCombatOverlay` outlines the Combat Phase's attackable hexes on the `combat` layer above the counters: red for mandatory WP objectives, orange for optional ones. Hexes already fought over get a small cross in the lower right.
