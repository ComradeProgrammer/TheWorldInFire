import { Container, Graphics } from "pixi.js";
import type { HexGrid } from "../hexGrid";
import type { HexData, MapData, Terrain } from "../mapTypes";
import { COLORS } from "./style";

/** Small deterministic PRNG so the procedural artwork is identical on every run. */
function rng(seed: string): () => number {
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return () => {
    h += 0x6d2b79f5;
    let t = h;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Circles on a small lattice around the centre. Drawing them for neighbouring
 * hexes of the same terrain merges into the soft blob shapes of the printed art.
 */
function blob(g: Graphics, x: number, y: number, radius: number, rand: () => number): void {
  g.circle(x, y, radius * 0.62);
  for (let i = 0; i < 6; i++) {
    const a = (Math.PI / 3) * i + Math.PI / 6;
    const r = radius * (0.5 + rand() * 0.08);
    g.circle(x + Math.cos(a) * radius * 0.48, y + Math.sin(a) * radius * 0.48, r);
  }
}

function pointInHex(grid: HexGrid, x: number, y: number, rand: () => number, spread = 0.82) {
  // Rejection-sample a point inside the inscribed ellipse of the hex.
  for (;;) {
    const u = rand() * 2 - 1;
    const v = rand() * 2 - 1;
    if (u * u + v * v <= 1) return { x: x + u * grid.halfWidth * spread, y: y + v * grid.radiusY * spread };
  }
}

function drawForestShadow(g: Graphics, grid: HexGrid, h: HexData) {
  const { x, y } = grid.center(h.row, h.col);
  blob(g, x, y, grid.halfWidth * 1.14, rng(`shadow${h.id}`));
  g.fill(COLORS.forestShadow);
}

function drawForest(g: Graphics, grid: HexGrid, h: HexData) {
  const { x, y } = grid.center(h.row, h.col);
  const rand = rng(h.id);
  blob(g, x, y, grid.halfWidth * 1.02, rand);
  g.fill(COLORS.forest);
  for (let i = 0; i < 22; i++) {
    const p = pointInHex(grid, x, y, rand);
    g.circle(p.x, p.y, 6 + rand() * 8);
  }
  g.fill({ color: COLORS.forestDark, alpha: 0.85 });
  for (let i = 0; i < 16; i++) {
    const p = pointInHex(grid, x, y, rand);
    g.circle(p.x, p.y, 4 + rand() * 5);
  }
  g.fill({ color: COLORS.forestLight, alpha: 0.8 });
}

function drawStreaks(
  g: Graphics,
  grid: HexGrid,
  h: HexData,
  base: number,
  streak: number,
  count: number,
  width: number,
) {
  const { x, y } = grid.center(h.row, h.col);
  const rand = rng(h.id);
  blob(g, x, y, grid.halfWidth * 1.04, rand);
  g.fill(base);
  const heading = rand() * Math.PI;
  for (let i = 0; i < count; i++) {
    const p = pointInHex(grid, x, y, rand, 0.78);
    const a = heading + (rand() - 0.5) * 0.9;
    const len = 12 + rand() * 18;
    const bend = (rand() - 0.5) * 10;
    const dx = Math.cos(a) * len;
    const dy = Math.sin(a) * len;
    g.moveTo(p.x - dx, p.y - dy);
    g.quadraticCurveTo(p.x - dy * 0.3 + bend, p.y + dx * 0.3 + bend, p.x + dx, p.y + dy);
  }
  g.stroke({ color: streak, width, cap: "round", alpha: 0.8 });
}

function drawMarsh(g: Graphics, grid: HexGrid, h: HexData) {
  const { x, y } = grid.center(h.row, h.col);
  const rand = rng(h.id);
  blob(g, x, y, grid.halfWidth * 1.0, rand);
  g.fill(COLORS.marsh);
  for (let i = 0; i < 20; i++) {
    const p = pointInHex(grid, x, y, rand, 0.75);
    const s = 5 + rand() * 4;
    g.moveTo(p.x - s * 0.6, p.y - s).lineTo(p.x, p.y);
    g.moveTo(p.x, p.y - s * 1.2).lineTo(p.x, p.y);
    g.moveTo(p.x + s * 0.6, p.y - s).lineTo(p.x, p.y);
    g.moveTo(p.x - s, p.y).lineTo(p.x + s, p.y);
  }
  g.stroke({ color: COLORS.marshGrass, width: 1.6, alpha: 0.75 });
}

/** Base land fill, then terrain artwork drawn in priority order so the richer art sits on top. */
export function buildTerrainLayer(map: MapData, grid: HexGrid): Container {
  const layer = new Container({ label: "terrain" });

  const base = new Graphics();
  for (const h of map.hexes) {
    base.poly(grid.corners(h.row, h.col, 1.005));
  }
  base.fill(COLORS.clear);
  for (const h of map.hexes) {
    if (h.terrain !== "sea") continue;
    base.poly(grid.corners(h.row, h.col, 1.005));
  }
  base.fill(COLORS.sea);
  layer.addChild(base);

  // Faint speckle so clear terrain is not a flat colour.
  const speck = new Graphics();
  for (const h of map.hexes) {
    if (h.terrain === "sea") continue;
    const rand = rng(`s${h.id}`);
    const { x, y } = grid.center(h.row, h.col);
    for (let i = 0; i < 10; i++) {
      const p = pointInHex(grid, x, y, rand, 0.9);
      speck.circle(p.x, p.y, 2 + rand() * 4);
    }
  }
  speck.fill({ color: COLORS.clearSpeck, alpha: 0.6 });
  layer.addChild(speck);

  // Opaque (pre-blended) shadow so overlapping shapes don't darken each other.
  const shadow = new Graphics();
  for (const h of map.hexes) if (h.terrain === "forest") drawForestShadow(shadow, grid, h);

  const order: Terrain[] = ["marsh", "forest", "rough", "mountain"];
  for (const terrain of order) {
    if (terrain === "forest") layer.addChild(shadow);
    const g = new Graphics();
    for (const h of map.hexes) {
      if (h.terrain !== terrain) continue;
      if (terrain === "marsh") drawMarsh(g, grid, h);
      else if (terrain === "forest") drawForest(g, grid, h);
      else if (terrain === "rough") drawStreaks(g, grid, h, COLORS.rough, COLORS.roughStreak, 9, 5);
      else drawStreaks(g, grid, h, COLORS.mountain, COLORS.mountainStreak, 14, 7);
    }
    layer.addChild(g);
  }
  return layer;
}
