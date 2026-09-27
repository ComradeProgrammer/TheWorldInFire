import type { GameEvent, StrikeResolution, UnitState } from "../gameApi";

/** One read-only line in the Combat Log pane. */
export interface CombatLogEntry {
  id: number;
  text: string;
  tone: "roll" | "hit" | "info";
}

const RESULT_LABELS: Record<StrikeResolution["result"], string> = {
  noEffect: "no effect",
  disrupted: "Disrupted",
  stepLoss: "step loss",
};

function sideLabel(sideId: string): string {
  return sideId === "nato" ? "NATO" : sideId === "warsawPact" ? "WP" : sideId;
}

function signed(value: number): string {
  return value >= 0 ? `+${value}` : `${value}`;
}

/**
 * Turns authoritative strike events into log lines. `units` must include units
 * eliminated by these events (pass the pre-command roster as well).
 */
export function describeCombatEvents(events: GameEvent[], units: UnitState[], firstId: number): CombatLogEntry[] {
  const name = (id: string) => units.find((unit) => unit.id === id)?.name ?? id;
  const lines: Omit<CombatLogEntry, "id">[] = [];
  for (const event of events) {
    switch (event.type) {
      case "airStrikeResolved": {
        const { dieRoll, modifier, modifiedRoll, result } = event.resolution;
        lines.push({
          tone: "roll",
          text: `${sideLabel(event.sideId)} air strike on ${event.hexId} (${event.unitIds.map(name).join(", ")}): rolled ${dieRoll} ${signed(modifier)} = ${modifiedRoll} → ${RESULT_LABELS[result]}`,
        });
        break;
      }
      case "unitStepLost":
        lines.push({ tone: "hit", text: `${name(event.unitId)} loses a step` });
        break;
      case "unitEliminated":
        lines.push({ tone: "hit", text: `${name(event.unitId)} eliminated in ${event.hexId}` });
        break;
      case "unitDisruptionChanged":
        if (event.disruption) {
          lines.push({ tone: "hit", text: `${name(event.unitId)} ${event.disruption === "suppressed" ? "Suppressed" : "Disrupted"}` });
        }
        break;
      case "breakthroughMarkerPlaced":
        lines.push({ tone: "info", text: `Breakthrough in ${event.hexId}` });
        break;
      case "airInterdictionZonePlaced":
        lines.push({ tone: "info", text: `${sideLabel(event.sideId)} Air Interdiction Zone at ${event.hexId}` });
        break;
      default:
        break;
    }
  }
  return lines.map((line, index) => ({ ...line, id: firstId + index }));
}
