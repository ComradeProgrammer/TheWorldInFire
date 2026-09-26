import { Container, Graphics, Text, type TextStyleOptions } from "pixi.js";
import type { UnitState } from "../../gameApi";
import type { HexGrid } from "../hexGrid";
import type { MapData } from "../mapTypes";
import { COLORS, FONT_FAMILY } from "./style";

const COUNTER_SIZE = 80;
const STACK_OFFSET = 11;
const MAX_VISIBLE_STACK = 3;

const SIDE_COLORS = {
  nato: { fill: 0x769bc5, edge: 0xc8e2ff, ink: 0x101923 },
  warsawPact: { fill: 0xc86a5b, edge: 0xffc2b6, ink: 0x24110e },
} as const;

const NATION_COLORS: Record<string, number> = {
  denmark: 0xd82735,
  eastGermany: 0x3f4650,
  poland: 0xf2f2f0,
  sovietUnion: 0xb5222c,
  unitedKingdom: 0x274d83,
  unitedStates: 0x4d6841,
  westGermany: 0xd8ab32,
};

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

function drawUnitSymbol(g: Graphics, unit: UnitState, ink: number): string | null {
  const type = unit.unitTypeId.toLowerCase();
  g.rect(-25, -21, 50, 32).stroke({ color: ink, width: 2.8 });

  if (type === "headquarters") return "HQ";
  if (type.includes("tank") || type.includes("armored")) {
    g.ellipse(0, -5, 17, 9).stroke({ color: ink, width: 2.8 });
    return null;
  }
  if (type.includes("mechanized") || type.includes("motorRifle".toLowerCase())) {
    g.moveTo(-23, -19).lineTo(23, 9).moveTo(23, -19).lineTo(-23, 9).stroke({ color: ink, width: 2.2 });
    g.ellipse(0, -5, 16, 8).stroke({ color: ink, width: 2.2 });
    return null;
  }
  if (type.includes("airborne") || type.includes("airmobile")) {
    g.moveTo(-22, -17).lineTo(22, 9).moveTo(22, -17).lineTo(-22, 9).stroke({ color: ink, width: 2.2 });
    g.moveTo(-15, -2).quadraticCurveTo(0, -19, 15, -2).stroke({ color: ink, width: 2.2 });
    return null;
  }
  if (type.includes("marine")) return "M";
  if (unit.traits.includes("territorial")) return "T";

  g.moveTo(-23, -19).lineTo(23, 9).moveTo(23, -19).lineTo(-23, 9).stroke({ color: ink, width: 2.8 });
  return null;
}

function buildCounter(unit: UnitState): Container {
  const palette = SIDE_COLORS[unit.sideId === "nato" ? "nato" : "warsawPact"];
  const counter = new Container({ label: unit.id });
  counter.cullable = true;
  const g = new Graphics();
  const half = COUNTER_SIZE / 2;
  g.roundRect(-half, -half, COUNTER_SIZE, COUNTER_SIZE, 5)
    .fill(palette.fill)
    .stroke({ color: 0x05080c, width: 5, alpha: 0.72 })
    .stroke({ color: palette.edge, width: 2 });
  g.rect(-half + 4, -half + 4, COUNTER_SIZE - 8, 8).fill(NATION_COLORS[unit.nationId] ?? palette.edge);
  const symbolText = drawUnitSymbol(g, unit, palette.ink);
  counter.addChild(g);
  if (symbolText) counter.addChild(counterText(symbolText, symbolText === "HQ" ? 17 : 21, -5, palette.ink));

  const step = unit.steps[unit.strengthStepIndex];
  if (step) counter.addChild(counterText(`${step.attack}  ${step.defense}  ${step.movement}`, 19, 27, palette.ink));
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
export function populateUnitLayer(layer: Container, units: UnitState[], map: MapData, grid: HexGrid): CounterHit[] {
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
      const counter = buildCounter(unit);
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
