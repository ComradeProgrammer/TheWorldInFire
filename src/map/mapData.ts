import rawMap from "./data/natoMap.json";
import { HexGrid } from "./hexGrid";
import type { HexData, MapData } from "./mapTypes";

export const natoMap = rawMap as unknown as MapData;

export const natoGrid = new HexGrid(natoMap.grid);

export const hexesById: ReadonlyMap<string, HexData> = new Map(natoMap.hexes.map((h) => [h.id, h]));

export const TERRAIN_NAMES: Record<HexData["terrain"], string> = {
  sea: "All-Sea",
  clear: "Clear",
  marsh: "Marsh",
  forest: "Forest",
  rough: "Rough",
  mountain: "Mountain",
};

export const CITY_KIND_NAMES = { minor: "Minor City", major: "Major City", key: "Key City" } as const;
