/**
 * Shape of the generated map description in `data/natoMap.json`.
 *
 * The file was traced once, offline, from the reference
 * VASSAL boards. All coordinates are "map pixels": the stitched board space in
 * which hex centres follow the grid parameters below.
 */

export type Terrain = "sea" | "clear" | "marsh" | "forest" | "rough" | "mountain";

export type CityKind = "minor" | "major" | "key";

export type CommandZone = "BALTAP" | "NORTHAG" | "CENTAG";

export interface MapGrid {
  hexWidth: number;
  rowSpacing: number;
  originX: number;
  originY: number;
  /** Printed column number of the leftmost grid column (columns count down to the right). */
  columnBase: number;
  width: number;
  height: number;
}

export interface CityData {
  name: string;
  kind: CityKind;
  /** Organic defense strength printed in the yellow box. */
  defense: number;
}

export interface HexData {
  /** Printed hex number, RRCC. */
  id: string;
  row: number;
  col: number;
  /** Natural terrain. A city in the hex takes priority for rules purposes. */
  terrain: Terrain;
  coastal?: boolean;
  city?: CityData;
  /** Port capacity. */
  port?: number;
  town?: string;
  mobilization?: boolean;
  commandZone?: CommandZone;
}

export type HexsideFeature = "corpsBoundary" | "frontBoundary" | "blocked";

export interface HexsideData {
  a: string;
  b: string;
  features: HexsideFeature[];
}

export type LineKind =
  | "majorRiver"
  | "minorRiver"
  | "nationalBoundary"
  | "ironCurtain"
  | "coast"
  | "seaBoundary";

/** Flat [x0, y0, x1, y1, ...] coordinate list. */
export type FlatPoints = number[];

export interface LineData {
  kind: LineKind;
  points: FlatPoints;
}

export interface WaterData {
  outer: FlatPoints;
  holes?: FlatPoints[];
}

export interface CommandLineData {
  name: string;
  north: CommandZone;
  south: CommandZone;
  points: FlatPoints;
}

export interface CityOutlineData {
  kind: CityKind;
  points: FlatPoints;
}

export interface CausewayData {
  points: FlatPoints;
  width: number;
}

export type SymbolData =
  | { kind: "defense"; hex: string; value: number; x: number; y: number }
  | { kind: "port"; hex: string; value: number; x: number; y: number }
  | { kind: "mobilization"; hex: string; x: number; y: number };

export type LabelKind =
  | "sea"
  | "island"
  | "river"
  | "command"
  | "deployment"
  | "deploymentPact"
  | "keyCity"
  | "majorCity"
  | "minorCity"
  | "town";

export interface LabelData {
  text: string;
  kind: LabelKind;
  x: number;
  y: number;
}

export interface MapData {
  version: number;
  source: string;
  grid: MapGrid;
  hexes: HexData[];
  symbols: SymbolData[];
  water: WaterData[];
  lines: LineData[];
  commandLines: CommandLineData[];
  cities: CityOutlineData[];
  causeways: CausewayData[];
  labels: LabelData[];
  hexsides: HexsideData[];
}
