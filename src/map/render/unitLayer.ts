import { Container, Graphics, Text, type TextStyleOptions } from "pixi.js";
import type { UnitState } from "../../gameApi";
import type { HexGrid } from "../hexGrid";
import type { MapData } from "../mapTypes";
import { COUNTER, counterPalette, FRAME, RESERVE_TAB, reserveTabLabel, NATION_COLORS, nationCode, nationInk, toCounter, unitSymbol, type SymbolPath } from "../unitSymbols";
import { COLORS, FONT_FAMILY } from "./style";

const COUNTER_SIZE = COUNTER.size;
const STACK_OFFSET = 11;
const MAX_VISIBLE_STACK = 3;

const DISRUPTION_COLOR = 0xff9f1c;

const TEXT_BASE: TextStyleOptions = {
  fontFamily: FONT_FAMILY,
  fontWeight: "bold",
  fill: 0x101923,
};

function counterText(text: string, size: number, y: number, fill = 0x101923): Text {
  const label = new Text({ text, style: { ...TEXT_BASE, fontSize: size, fill }, resolution: 3 });
  label.anchor.set(0.5);
  label.position.set(0, y);
  return label;
}

function tracePaths(g: Graphics, paths: SymbolPath[]): void {
  for (const path of paths) {
    for (const [command, ...values] of path) {
      const points = [] as number[];
      for (let i = 0; i < values.length; i += 2) {
        const p = toCounter(values[i], values[i + 1]);
        points.push(p.x, p.y);
      }
      if (command === "M") g.moveTo(points[0], points[1]);
      else if (command === "L") g.lineTo(points[0], points[1]);
      else g.bezierCurveTo(points[0], points[1], points[2], points[3], points[4], points[5]);
    }
  }
}

/** Draws the unit's APP-6 symbol: frame, branch icon, modifier, echelon, and HQ staff. */
function drawUnitSymbol(g: Graphics, unit: UnitState, ink: number): string | null {
  const symbol = unitSymbol(unit);
  const topLeft = toCounter(FRAME.x1, FRAME.y1);
  const bottomRight = toCounter(FRAME.x2, FRAME.y2);
  g.rect(topLeft.x, topLeft.y, bottomRight.x - topLeft.x, bottomRight.y - topLeft.y).stroke({ color: ink, width: 2.4 });
  if (symbol.strokes.length > 0) {
    tracePaths(g, symbol.strokes);
    g.stroke({ color: ink, width: 2, cap: "round", join: "round" });
  }
  if (symbol.echelon.length > 0) {
    tracePaths(g, symbol.echelon);
    g.stroke({ color: ink, width: 1.8, cap: "round" });
  }
  if (symbol.headquarters) {
    g.moveTo(topLeft.x, bottomRight.y).lineTo(topLeft.x, bottomRight.y + COUNTER.staffLength).stroke({ color: ink, width: 2.4 });
  }
  return symbol.label;
}

function buildCounter(unit: UnitState, reserve: boolean): Container {
  const palette = counterPalette(unit.sideId);
  const counter = new Container({ label: unit.id });
  counter.cullable = true;
  const g = new Graphics();
  const half = COUNTER_SIZE / 2;
  g.roundRect(-half, -half, COUNTER_SIZE, COUNTER_SIZE, 5)
    .fill(palette.fill)
    .stroke({ color: 0x05080c, width: 5, alpha: 0.72 })
    .stroke({ color: palette.edge, width: 2 });
  const nationColor = NATION_COLORS[unit.nationId] ?? palette.edge;
  const { band } = COUNTER;
  g.rect(band.x, band.y, band.width, band.height).fill(nationColor);
  const label = drawUnitSymbol(g, unit, palette.ink);
  counter.addChild(g);
  if (label) counter.addChild(counterText(label, 14, toCounter(100, 100).y, palette.ink));
  counter.addChild(counterText(nationCode(unit.nationId), 10, band.y + band.height / 2, nationInk(nationColor)));

  const step = unit.steps[unit.strengthStepIndex];
  if (step) counter.addChild(counterText(`${step.attack}  ${step.defense}  ${step.movement}`, 18, COUNTER.valuesY, palette.ink));

  // Disrupted / Suppressed marker badge in the lower-left corner.
  if (unit.disruption) {
    const badge = new Graphics()
      .circle(-half + 6, half - 6, 15)
      .fill(DISRUPTION_COLOR)
      .stroke({ color: 0x05080c, width: 3 });
    const label = counterText(unit.disruption === "suppressed" ? "S" : "D", 17, half - 6, 0x05080c);
    label.x = -half + 6;
    counter.addChild(badge, label);
  }

  // Reserve/OMG Marker: a tab on the bottom edge, like a marker placed on the unit.
  if (reserve) {
    const { reserveTab: tab } = COUNTER;
    const marker = new Graphics()
      .roundRect(-tab.width / 2, tab.y, tab.width, tab.height, 3)
      .fill(RESERVE_TAB.fill)
      .stroke({ color: 0x05080c, width: 2 });
    counter.addChild(marker, counterText(reserveTabLabel(unit.sideId), 11, tab.y + tab.height / 2, RESERVE_TAB.ink));
  }
  return counter;
}

/** World-space footprint of one drawn counter, used for click hit testing. */
export interface CounterHit {
  unitId: string;
  x: number;
  y: number;
}

/** Returns the top-most counter under a world point, if any. */
export function counterAt(hits: CounterHit[], wx: number, wy: number): string | null {
  const half = COUNTER_SIZE / 2;
  for (let i = hits.length - 1; i >= 0; i--) {
    const hit = hits[i];
    if (Math.abs(wx - hit.x) <= half && Math.abs(wy - hit.y) <= half) return hit.unitId;
  }
  return null;
}

/**
 * Rebuilds the programmatically drawn counter layer from authoritative units.
 * Returns counter footprints in draw order (last is top-most).
 */
export function populateUnitLayer(
  layer: Container,
  units: UnitState[],
  map: MapData,
  grid: HexGrid,
  reserveUnitIds: ReadonlySet<string>,
): CounterHit[] {
  for (const child of layer.removeChildren()) child.destroy({ children: true });
  const hits: CounterHit[] = [];

  const mapHexes = new Map(map.hexes.map((hex) => [hex.id, hex]));
  const stacks = new Map<string, UnitState[]>();
  for (const unit of units) {
    if (unit.location.type !== "hex") continue;
    const stack = stacks.get(unit.location.hexId) ?? [];
    stack.push(unit);
    stacks.set(unit.location.hexId, stack);
  }

  for (const [hexId, stack] of stacks) {
    const hex = mapHexes.get(hexId);
    if (!hex) continue;
    stack.sort((a, b) => a.id.localeCompare(b.id));
    const visible = stack.slice(-MAX_VISIBLE_STACK);
    const center = grid.center(hex.row, hex.col);
    visible.forEach((unit, index) => {
      const counter = buildCounter(unit, reserveUnitIds.has(unit.id));
      const offset = (index - (visible.length - 1) / 2) * STACK_OFFSET;
      counter.position.set(center.x + offset, center.y - offset);
      layer.addChild(counter);
      hits.push({ unitId: unit.id, x: counter.x, y: counter.y });
    });

    if (stack.length > 1) {
      const badge = new Container();
      badge.position.set(center.x + 40, center.y - 40);
      const circle = new Graphics().circle(0, 0, 15).fill(COLORS.ink).stroke({ color: 0xffffff, width: 1.8 });
      badge.addChild(circle, counterText(String(stack.length), 16, 0, 0xffffff));
      layer.addChild(badge);
    }
  }
  return hits;
}
