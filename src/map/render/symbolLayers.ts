import { BitmapFont, BitmapText, Container, Graphics, Text, type TextStyleOptions } from "pixi.js";
import type { HexGrid } from "../hexGrid";
import type { CityKind, LabelKind, MapData } from "../mapTypes";
import { COLORS, FONT_FAMILY } from "./style";

const CITY_FILL: Record<CityKind, [number, number]> = {
  minor: [COLORS.minorCity, COLORS.minorCityEdge],
  major: [COLORS.majorCity, COLORS.majorCityEdge],
  key: [COLORS.keyCity, COLORS.keyCityEdge],
};

export function buildCityLayer(map: MapData): Container {
  const layer = new Container({ label: "cities" });
  const g = new Graphics();
  for (const kind of ["minor", "major", "key"] as const) {
    const [fill, edge] = CITY_FILL[kind];
    for (const c of map.cities) if (c.kind === kind) g.poly(c.points);
    g.fill(fill);
    for (const c of map.cities) if (c.kind === kind) g.poly(c.points);
    g.stroke({ color: edge, width: 6, alpha: 0.18, join: "round" });
    for (const c of map.cities) if (c.kind === kind) g.poly(c.points);
    g.stroke({ color: edge, width: 1.8, join: "round" });
  }
  layer.addChild(g);
  return layer;
}

function anchor(g: Graphics, x: number, y: number): void {
  g.circle(x, y - 11, 3);
  g.moveTo(x, y - 8).lineTo(x, y + 5);
  g.moveTo(x - 6, y - 4).lineTo(x + 6, y - 4);
  g.moveTo(x - 9, y).quadraticCurveTo(x - 7, y + 7, x, y + 6).quadraticCurveTo(x + 7, y + 7, x + 9, y);
}

export function buildSymbolLayer(map: MapData): Container {
  const layer = new Container({ label: "symbols" });
  const g = new Graphics();
  const numbers = new Container();
  const numberStyle: TextStyleOptions = { fontFamily: FONT_FAMILY, fontWeight: "bold", fontSize: 24, fill: COLORS.ink };

  for (const s of map.symbols) {
    if (s.kind === "mobilization") {
      g.poly([s.x, s.y - 11, s.x + 12, s.y + 10, s.x - 12, s.y + 10])
        .fill(COLORS.mobilization)
        .stroke({ color: COLORS.ink, width: 1.5 });
    } else if (s.kind === "defense") {
      g.rect(s.x - 12, s.y - 13, 24, 26).fill(COLORS.defenseBox).stroke({ color: COLORS.ink, width: 1.2 });
      const t = new Text({ text: String(s.value), style: numberStyle, resolution: 3 });
      t.anchor.set(0.5);
      t.position.set(s.x, s.y + 1);
      numbers.addChild(t);
    } else {
      g.circle(s.x, s.y, 19).fill(COLORS.ink).stroke({ color: COLORS.port, width: 2 });
      anchor(g, s.x, s.y - 3);
      g.stroke({ color: COLORS.port, width: 2, cap: "round" });
      const t = new Text({
        text: String(s.value),
        style: { ...numberStyle, fontSize: 17, fill: COLORS.port },
        resolution: 3,
      });
      t.anchor.set(0.5);
      t.position.set(s.x, s.y + 11);
      numbers.addChild(t);
    }
  }
  layer.addChild(g, numbers);
  return layer;
}

const LABEL_STYLE: Record<LabelKind, TextStyleOptions> = {
  sea: { fontSize: 44, fontWeight: "bold", fill: 0x4fa6d6, letterSpacing: 4, fontStyle: "italic" },
  island: { fontSize: 35, fontWeight: "bold", fill: 0xcfe6f5, fontStyle: "italic" },
  river: { fontSize: 25, fill: 0x62b6ff, fontStyle: "italic" },
  command: { fontSize: 35, fontWeight: "bold", fill: 0xb58cff, letterSpacing: 2 },
  deployment: { fontSize: 48, fontWeight: "bold", fill: 0x6f9dff },
  deploymentPact: { fontSize: 48, fontWeight: "bold", fill: 0xff6b5c },
  keyCity: { fontSize: 35, fontWeight: "bold", fill: 0xffffff, letterSpacing: 1 },
  majorCity: { fontSize: 26, fontWeight: "bold", fill: 0xe4ecf3 },
  minorCity: { fontSize: 23, fill: 0xb9c6d2 },
  town: { fontSize: 23, fill: 0x7ec8f0, fontStyle: "italic" },
};

/** Dark halo keeps light text legible over any terrain. */
const LABEL_HALO = { color: COLORS.labelHalo, width: 6, join: "round" } as const;

/** Labels grouped by importance so the renderer can hide small text when zoomed out. */
export interface LabelLayers {
  major: Container;
  minor: Container;
}

export function buildLabelLayers(map: MapData): LabelLayers {
  const major = new Container({ label: "labels-major" });
  const minor = new Container({ label: "labels-minor" });
  for (const l of map.labels) {
    const style = { fontFamily: FONT_FAMILY, stroke: LABEL_HALO, ...LABEL_STYLE[l.kind] };
    const text = l.kind === "keyCity" ? l.text.toUpperCase() : l.text;
    const t = new Text({ text, style, resolution: 3 });
    t.anchor.set(0.5);
    t.position.set(l.x, l.y);
    t.cullable = true;
    const isMajor = ["sea", "island", "command", "deployment", "deploymentPact", "keyCity", "majorCity"].includes(l.kind);
    (isMajor ? major : minor).addChild(t);
  }
  return { major, minor };
}

let hexFontInstalled = false;

export function buildHexNumberLayer(map: MapData, grid: HexGrid): Container {
  if (!hexFontInstalled) {
    BitmapFont.install({
      name: "HexNumbers",
      style: { fontFamily: FONT_FAMILY, fontSize: 64, fill: 0xffffff },
      chars: [["0", "9"]],
      resolution: 1,
    });
    hexFontInstalled = true;
  }
  const layer = new Container({ label: "hex-numbers" });
  for (const h of map.hexes) {
    const { x, y } = grid.center(h.row, h.col);
    const t = new BitmapText({ text: h.id, style: { fontFamily: "HexNumbers", fontSize: 18 } });
    t.tint = h.terrain === "sea" ? 0x3f7fae : 0x8aa6b8;
    t.alpha = 0.6;
    t.anchor.set(0.5, 0);
    t.position.set(x, y - grid.radiusY + 12);
    t.cullable = true;
    layer.addChild(t);
  }
  return layer;
}

/**
 * Reinforcement Sector entry hexes (house rule replacing the printed boxes):
 * an "R1".."R5" badge in the side's colour at the bottom of each entry hex.
 */
export function buildSectorLayer(map: MapData, grid: HexGrid): Container {
  const layer = new Container({ label: "reinforcement-sectors" });
  const g = new Graphics();
  const labels = new Container();
  const byId = new Map(map.hexes.map((hex) => [hex.id, hex]));
  for (const sector of map.reinforcementSectors ?? []) {
    const hex = byId.get(sector.hexId);
    if (!hex) continue;
    const { x, y } = grid.center(hex.row, hex.col);
    const top = y + grid.radiusY * 0.42;
    const color = sector.sideId === "nato" ? 0x6fa3e0 : 0xe06a5c;
    g.roundRect(x - 22, top, 44, 22, 4).fill({ color: COLORS.ink, alpha: 0.85 }).stroke({ color, width: 2.5 });
    const label = new Text({
      text: `R${sector.number}`,
      style: { fontFamily: FONT_FAMILY, fontWeight: "bold", fontSize: 16, fill: color },
      resolution: 3,
    });
    label.anchor.set(0.5);
    label.position.set(x, top + 11);
    labels.addChild(label);
  }
  layer.addChild(g, labels);
  return layer;
}
