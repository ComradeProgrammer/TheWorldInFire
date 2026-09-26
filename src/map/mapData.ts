import type { HexData } from "./mapTypes";

export const TERRAIN_NAMES: Record<HexData["terrain"], string> = {
  sea: "All-Sea",
  clear: "Clear",
  marsh: "Marsh",
  forest: "Forest",
  rough: "Rough",
  mountain: "Mountain",
};

export const CITY_KIND_NAMES = { minor: "Minor City", major: "Major City", key: "Key City" } as const;
