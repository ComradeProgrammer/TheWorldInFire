import type { UnitState } from "../gameApi";

export function sideName(sideId: string): string {
  return sideId === "nato" ? "NATO" : sideId === "warsawPact" ? "Warsaw Pact" : sideId;
}

/** `camelCase` identifier as readable words: `motorRifleDivision` → `Motor Rifle Division`. */
export function readableId(value: string): string {
  return value
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/^./, (letter) => letter.toUpperCase());
}

/** Short type glyph drawn on counters and unit chips. */
export function unitSymbol(unit: UnitState): string {
  const type = unit.unitTypeId.toLowerCase();
  if (type === "headquarters") return "HQ";
  if (type.includes("tank") || type.includes("armored")) return "◉";
  if (type.includes("mechanized") || type.includes("motorrifle")) return "ⓧ";
  if (type.includes("airborne") || type.includes("airmobile")) return "⌁";
  if (type.includes("marine")) return "M";
  return "╳";
}
