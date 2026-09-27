export function sideName(sideId: string): string {
  return sideId === "nato" ? "NATO" : sideId === "warsawPact" ? "Warsaw Pact" : sideId;
}

/** `camelCase` identifier as readable words: `motorRifleDivision` → `Motor Rifle Division`. */
export function readableId(value: string): string {
  return value
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/^./, (letter) => letter.toUpperCase());
}
