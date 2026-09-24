import { Container, Graphics } from "pixi.js";
import type { HexGrid } from "../hexGrid";
import { parseHexId } from "../hexGrid";
import type { FlatPoints, HexsideFeature, LineKind, MapData } from "../mapTypes";
import { COLORS } from "./style";

function path(g: Graphics, pts: FlatPoints): void {
  g.moveTo(pts[0], pts[1]);
  for (let i = 2; i < pts.length; i += 2) g.lineTo(pts[i], pts[i + 1]);
}

/**
 * Emit dashes along a polyline. `pattern` alternates on/off lengths; each "on"
 * run becomes its own sub-path so a single stroke() draws the whole pattern.
 */
export function dashed(g: Graphics, pts: FlatPoints, pattern: number[]): void {
  let idx = 0;
  let remaining = pattern[0];
  let on = true;
  let penDown = false;
  for (let i = 0; i + 3 < pts.length; i += 2) {
    let x = pts[i];
    let y = pts[i + 1];
    const x2 = pts[i + 2];
    const y2 = pts[i + 3];
    let segLen = Math.hypot(x2 - x, y2 - y);
    if (segLen === 0) continue;
    const ux = (x2 - x) / segLen;
    const uy = (y2 - y) / segLen;
    while (segLen > 0) {
      const step = Math.min(remaining, segLen);
      const nx = x + ux * step;
      const ny = y + uy * step;
      if (on) {
        if (!penDown) {
          g.moveTo(x, y);
          penDown = true;
        }
        g.lineTo(nx, ny);
      }
      x = nx;
      y = ny;
      segLen -= step;
      remaining -= step;
      if (remaining <= 1e-6) {
        idx = (idx + 1) % pattern.length;
        remaining = pattern[idx];
        on = !on;
        penDown = false;
      }
    }
  }
}

function linesOf(map: MapData, kind: LineKind): FlatPoints[] {
  return map.lines.filter((l) => l.kind === kind).map((l) => l.points);
}

export function buildWaterLayer(map: MapData): Container {
  const layer = new Container({ label: "water" });
  const water = new Graphics();
  for (const w of map.water) {
    water.poly(w.outer).fill(COLORS.sea);
    for (const hole of w.holes ?? []) water.poly(hole).cut();
  }
  layer.addChild(water);

  const coast = new Graphics();
  for (const pts of linesOf(map, "coast")) path(coast, pts);
  coast.stroke({ color: COLORS.sandEdge, width: 11, cap: "round", join: "round" });
  for (const pts of linesOf(map, "coast")) path(coast, pts);
  coast.stroke({ color: COLORS.sand, width: 7.5, cap: "round", join: "round" });
  layer.addChild(coast);
  return layer;
}

export function buildRiverLayer(map: MapData): Container {
  const layer = new Container({ label: "rivers" });
  const g = new Graphics();
  const minor = linesOf(map, "minorRiver");
  const major = linesOf(map, "majorRiver");
  for (const pts of minor) path(g, pts);
  g.stroke({ color: COLORS.minorRiverEdge, width: 8, cap: "round", join: "round" });
  for (const pts of minor) path(g, pts);
  g.stroke({ color: COLORS.minorRiver, width: 5, cap: "round", join: "round" });
  for (const pts of major) path(g, pts);
  g.stroke({ color: COLORS.majorRiverEdge, width: 17, cap: "round", join: "round" });
  for (const pts of major) path(g, pts);
  g.stroke({ color: COLORS.majorRiver, width: 11.5, cap: "round", join: "round" });
  layer.addChild(g);
  return layer;
}

export function buildGridLayer(map: MapData, grid: HexGrid): Container {
  const layer = new Container({ label: "grid" });
  const land = new Graphics();
  const sea = new Graphics();
  for (const h of map.hexes) {
    (h.terrain === "sea" ? sea : land).poly(grid.corners(h.row, h.col));
  }
  land.stroke({ color: COLORS.grid, width: 1.8, alpha: 0.75 });
  sea.stroke({ color: COLORS.seaGrid, width: 1.8, alpha: 0.8 });
  layer.addChild(sea, land);
  return layer;
}

export function buildBoundaryLayer(map: MapData): Container {
  const layer = new Container({ label: "boundaries" });
  const g = new Graphics();
  for (const pts of linesOf(map, "nationalBoundary")) dashed(g, pts, [24, 8, 6, 8]);
  g.stroke({ color: COLORS.boundary, width: 6.5, cap: "butt" });

  for (const pts of linesOf(map, "ironCurtain")) path(g, pts);
  g.stroke({ color: COLORS.ironCurtain, width: 16, cap: "round", join: "round", alpha: 0.95 });
  for (const pts of linesOf(map, "ironCurtain")) dashed(g, pts, [22, 7, 5, 7]);
  g.stroke({ color: COLORS.boundary, width: 5, cap: "butt" });
  layer.addChild(g);
  return layer;
}

function hexsideSegments(map: MapData, grid: HexGrid, feature: HexsideFeature): FlatPoints[] {
  const out: FlatPoints[] = [];
  for (const side of map.hexsides) {
    if (!side.features.includes(feature)) continue;
    const seg = grid.sharedSide(parseHexId(side.a), parseHexId(side.b));
    if (seg) out.push(seg);
  }
  return out;
}

export function buildBlockedLayer(map: MapData, grid: HexGrid): Container {
  const layer = new Container({ label: "blocked" });
  const g = new Graphics();
  for (const seg of hexsideSegments(map, grid, "blocked")) path(g, seg);
  g.stroke({ color: COLORS.blocked, width: 12, cap: "butt", alpha: 0.95 });
  layer.addChild(g);
  return layer;
}

export function buildDeploymentLayer(map: MapData, grid: HexGrid): Container {
  const layer = new Container({ label: "deployment" });
  const g = new Graphics();
  for (const [feature, color] of [
    ["corpsBoundary", COLORS.corps],
    ["frontBoundary", COLORS.front],
  ] as const) {
    const segs = hexsideSegments(map, grid, feature);
    for (const seg of segs) path(g, seg);
    g.stroke({ color: 0xffffff, width: 8, alpha: 0.75, cap: "butt" });
    for (const seg of segs) dashed(g, seg, [8, 7]);
    g.stroke({ color, width: 8, cap: "butt" });
  }
  layer.addChild(g);
  return layer;
}

export function buildCommandLayer(map: MapData): Container {
  const layer = new Container({ label: "command" });
  const g = new Graphics();
  for (const line of map.commandLines) path(g, line.points);
  for (const pts of linesOf(map, "seaBoundary")) path(g, pts);
  g.stroke({ color: COLORS.commandZone, width: 22, alpha: 0.55, join: "round", cap: "butt" });
  layer.addChild(g);
  return layer;
}

export function buildCausewayLayer(map: MapData): Container {
  const layer = new Container({ label: "causeways" });
  const g = new Graphics();
  for (const c of map.causeways) {
    const [x1, y1, x2, y2] = c.points;
    g.moveTo(x1, y1).lineTo(x2, y2).stroke({ color: COLORS.ink, width: c.width + 3 });
    g.moveTo(x1, y1).lineTo(x2, y2).stroke({ color: 0xffffff, width: c.width });
  }
  layer.addChild(g);
  return layer;
}
