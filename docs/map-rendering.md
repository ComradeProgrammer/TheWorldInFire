# Map rendering decision

Date: 2026-09-23

## Decision

- **Renderer:** the map is drawn with PixiJS 8 (`pixi.js`), inside a React host component (`src/map/MapCanvas.tsx`).
- **No map images:** the map is not a bitmap. Everything is drawn from vector data in `src/map/data/natoMap.json`, which was traced once from the reference VASSAL boards by a local, uncommitted extraction script.

## Coordinates

- **Map pixels:** world coordinates are the stitched VASSAL board space, 5651 × 7243 px.
- **Hex centres:** `x = 15 + 133.5·(38 − col) + (row odd ? 66.75 : 0)`, `y = −3 + 114.4·row`.
- **Hex ids:** the printed `RRCC` numbers are the stable hex ids. Commands to the Rust core should use these ids, not screen positions.

`src/map/hexGrid.ts` implements this geometry. The future Rust core should adopt the same hex ids and adjacency rules.

## Data ownership

- **Rules input:** `natoMap.json` is terrain reference data. The fields the rules need are:
  - `hexes[].terrain`
  - `city`
  - `port`
  - `coastal`
  - `commandZone`
  - `hexsides`

  These should later move into, or be generated for, the Rust core so the core stays authoritative.
- **Presentation only:** `water`, `lines`, `cities` outlines, `symbols` and `labels`.
- **Primary terrain:** `terrain` is the natural terrain. A city in the hex takes priority (TEC priority 1–2). That priority is resolved by the rules, not in the data.

## Renderer structure

| Module | Responsibility |
| --- | --- |
| `render/terrainLayer.ts` | Procedural terrain art (seeded per hex, deterministic) |
| `render/featureLayers.ts` | Water, coast, rivers, grid, borders, hexside features, command zones |
| `render/symbolLayers.ts` | Cities, defense boxes, ports, mobilization sites, labels, hex numbers |
| `render/camera.ts` | Pan and zoom (local view state only) |
| `render/MapRenderer.ts` | Application lifecycle, layers, level of detail, hit testing, hover and selection |

Hover, selection and camera are frontend-only state. No IPC happens during pointer movement.
