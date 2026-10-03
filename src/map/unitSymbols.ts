import type { UnitState } from "../gameApi";

/**
 * NATO APP-6 friendly land-unit symbols, drawn from vector parts so the map
 * renderer and the React UI show identical counters.
 *
 * Coordinates use the standard 200 × 200 symbol space, in which the friendly
 * (rectangular) frame spans x 25–175 and y 50–150.
 */
export const FRAME = { x1: 25, y1: 50, x2: 175, y2: 150 } as const;

export type PathCommand =
  | ["M", number, number]
  | ["L", number, number]
  | ["C", number, number, number, number, number, number];

/** One stroked sub-path. */
export type SymbolPath = PathCommand[];

export type Echelon = "regiment" | "brigade" | "division";

export interface UnitSymbol {
  /** Branch icon and mobility modifier, stroked inside the frame. */
  strokes: SymbolPath[];
  /** Echelon marks above the frame. */
  echelon: SymbolPath[];
  /** Headquarters staff, down from the frame's lower-left corner. */
  headquarters: boolean;
  /** Text printed in the frame's centre, e.g. `HQ`. */
  label: string | null;
}

const line = (x1: number, y1: number, x2: number, y2: number): SymbolPath => [["M", x1, y1], ["L", x2, y2]];

/** Infantry: the frame's diagonals. */
const INFANTRY: SymbolPath[] = [line(25, 50, 175, 150), line(25, 150, 175, 50)];

/** Armour: a track outline. */
const ARMOUR: SymbolPath[] = [[
  ["M", 75, 80],
  ["L", 125, 80],
  ["C", 150, 80, 150, 120, 125, 120],
  ["L", 75, 120],
  ["C", 50, 120, 50, 80, 75, 80],
]];

/** Airborne (sector 2 modifier): parachute canopy on the bottom edge. */
const AIRBORNE: SymbolPath[] = [[["M", 75, 140], ["C", 75, 125, 100, 125, 100, 140], ["C", 100, 125, 125, 125, 125, 140]]];

/** Air assault / airmobile (sector 2 modifier): rotor "V" on the bottom edge. */
const AIR_ASSAULT: SymbolPath[] = [[["M", 85, 125], ["L", 100, 145], ["L", 115, 125]]];

/** Amphibious (sector 2 modifier): a wave along the bottom of the frame. */
const AMPHIBIOUS: SymbolPath[] = (() => {
  const path: SymbolPath = [["M", 25, 145]];
  const width = 150 / 8;
  let [x, y] = [25, 145];
  for (let i = 0; i < 8; i++) {
    const next = y === 145 ? 125 : 145;
    path.push(["C", x + width, y, x, next, x + width, next]);
    x += width;
    y = next;
  }
  return [path];
})();

const cross = (x: number): SymbolPath[] => [line(x, 40, x + 25, 15), line(x + 25, 40, x, 15)];

const ECHELONS: Record<Echelon, SymbolPath[]> = {
  regiment: [line(80, 40, 80, 15), line(100, 40, 100, 15), line(120, 40, 120, 15)],
  brigade: cross(87.5),
  division: [...cross(70), ...cross(105)],
};

function echelonOf(unitTypeId: string): Echelon | null {
  const type = unitTypeId.toLowerCase();
  if (type.endsWith("regiment")) return "regiment";
  if (type.endsWith("brigade")) return "brigade";
  if (type.endsWith("division")) return "division";
  return null;
}

/** The APP-6 symbol for a unit, from its type and traits. */
export function unitSymbol(unit: UnitState): UnitSymbol {
  const type = unit.unitTypeId.toLowerCase();
  const echelon = echelonOf(unit.unitTypeId);
  const echelonMarks = echelon ? ECHELONS[echelon] : [];
  if (type === "headquarters" || unit.traits.includes("headquarters")) {
    return { strokes: [], echelon: echelonMarks, headquarters: true, label: "HQ" };
  }

  let strokes: SymbolPath[];
  if (type.includes("tank") || type.includes("armored") || type.includes("panzer")) strokes = ARMOUR;
  else if (type.includes("mechanized") || type.includes("motorrifle")) strokes = [...INFANTRY, ...ARMOUR];
  else strokes = INFANTRY;

  if (unit.traits.includes("airborne")) strokes = [...strokes, ...AIRBORNE];
  else if (unit.traits.includes("airmobile")) strokes = [...strokes, ...AIR_ASSAULT];
  else if (unit.traits.includes("marine")) strokes = [...strokes, ...AMPHIBIOUS];

  return { strokes, echelon: echelonMarks, headquarters: false, label: null };
}

/** SVG path data for symbol paths, in symbol space. */
export function svgPathData(paths: SymbolPath[]): string {
  return paths
    .map((path) => path.map(([command, ...values]) => `${command}${values.join(",")}`).join(" "))
    .join(" ");
}

// ------------------------------------------------------------------ counter layout

/**
 * Shared layout of an 80 × 80 counter centred on the origin: nation band on
 * top, echelon marks, the symbol frame, and the strength line.
 */
export const COUNTER = {
  size: 80,
  /** Symbol space → counter space: x' = (x - 100) · scale, y' = (y - 100) · scale + symbolY. */
  symbolScale: 0.28,
  symbolY: 1,
  band: { x: -36, y: -36, width: 72, height: 11 },
  staffLength: 6,
  valuesY: 27,
  /** Reserve/OMG Marker tab straddling the counter's bottom edge. */
  reserveTab: { width: 34, height: 14, y: 33 },
  /** Out of Supply tab above the counter's top edge, clear of the nation band. */
  supplyTab: { width: 34, height: 13, y: -49 },
} as const;

/** Out of Supply tab colours. */
export const SUPPLY_TAB = { fill: 0xff3b30, ink: 0xffffff } as const;

/** Whether the core reports any of the unit's supply types as out of supply. */
export function outOfSupply(unit: UnitState): boolean {
  const { headquarters, movement, combat } = unit.supply;
  return [headquarters, movement, combat].includes("outOfSupply");
}

/** Reserve/OMG Marker tab colours. */
export const RESERVE_TAB = { fill: 0xf1d48c, ink: 0x101923 } as const;

/** Short label on a counter's Reserve/OMG tab. */
export function reserveTabLabel(sideId: string): string {
  return sideId === "nato" ? "RES" : "OMG";
}

/** Maps a symbol-space point onto the counter. */
export function toCounter(x: number, y: number): { x: number; y: number } {
  return {
    x: (x - 100) * COUNTER.symbolScale,
    y: (y - 100) * COUNTER.symbolScale + COUNTER.symbolY,
  };
}

/** Counter colours per side: face, edge highlight, and ink. */
export const COUNTER_PALETTES = {
  nato: { fill: 0x769bc5, edge: 0xc8e2ff, ink: 0x101923 },
  warsawPact: { fill: 0xc86a5b, edge: 0xffc2b6, ink: 0x24110e },
} as const;

export function counterPalette(sideId: string) {
  return COUNTER_PALETTES[sideId === "nato" ? "nato" : "warsawPact"];
}

// ------------------------------------------------------------------ nations

export const NATION_COLORS: Record<string, number> = {
  denmark: 0xd82735,
  eastGermany: 0x3f4650,
  poland: 0xf2f2f0,
  sovietUnion: 0xb5222c,
  unitedKingdom: 0x274d83,
  unitedStates: 0x4d6841,
  westGermany: 0xd8ab32,
  netherlands: 0xe8742c,
  belgium: 0x1d1d1b,
  canada: 0xc8102e,
  france: 0x2b4c9a,
  czechoslovakia: 0x2a5aa8,
};

const NATION_CODES: Record<string, string> = {
  sovietUnion: "USSR",
  eastGermany: "GDR",
  poland: "PL",
  czechoslovakia: "CSSR",
  westGermany: "GER",
  unitedStates: "US",
  unitedKingdom: "UK",
  denmark: "DK",
  netherlands: "NLD",
  belgium: "BEL",
  canada: "CAN",
  france: "FRA",
};

/** Short nation code printed on counters, e.g. `USSR`, `GER`, `DK`. */
export function nationCode(nationId: string): string {
  return NATION_CODES[nationId] ?? nationId.slice(0, 3).toUpperCase();
}

/** Black or white, whichever reads better on the nation colour. */
export function nationInk(color: number): number {
  const r = (color >> 16) & 0xff;
  const g = (color >> 8) & 0xff;
  const b = color & 0xff;
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? 0x101923 : 0xffffff;
}
