import { ODDS_COLUMNS, type AirBaseState, type AirCombatPairing, type AirUnitState, type GameEvent, type StrikeResolution, type UnitState } from "../gameApi";

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

const AIR_RESULT_LABELS = {
  noEffect: "no effect",
  abort: "abort",
  damagedAbort: "step loss + abort",
  destroyedAbort: "destroyed + abort",
  destroyedAndDamagedAbort: "destroyed + damage + abort",
} as const;

/**
 * Turns authoritative combat, marker, and cleanup events into log lines. `units` must include units
 * eliminated by these events (pass the pre-command roster as well).
 */
export function describeCombatEvents(
  events: GameEvent[],
  units: UnitState[],
  airUnits: AirUnitState[],
  airBases: AirBaseState[],
  firstId: number,
): CombatLogEntry[] {
  const name = (id: string) => units.find((unit) => unit.id === id)?.name ?? id;
  const airName = (id: string) => airUnits.find((unit) => unit.id === id)?.name ?? id;
  const baseName = (id: string) => airBases.find((base) => base.id === id)?.name ?? id;
  const pairingText = (pairing: AirCombatPairing) => [pairing.first, pairing.second].map((attack) =>
    `${airName(attack.attackerId)} d20 ${attack.dieRoll}${attack.modifier ? ` ${signed(attack.modifier)}` : ""} on C${signed(attack.column)} → ${AIR_RESULT_LABELS[attack.result]}`,
  ).join("; ");
  const lines: Omit<CombatLogEntry, "id">[] = [];
  // The phase whose automatic work produced the following events, if it started in this batch.
  let phase: string | null = null;
  for (const event of events) {
    switch (event.type) {
      case "phaseStarted":
        phase = event.step.phaseId;
        break;
      case "unitSupplyChanged": {
        const { headquarters, movement, combat } = event.supply;
        const out = [headquarters, movement, combat].includes("outOfSupply");
        lines.push({ tone: out ? "hit" : "info", text: `${name(event.unitId)} ${out ? "is out of supply" : "is back in supply"}` });
        break;
      }
      case "reserveMarkersRemoved":
        lines.push({
          tone: "info",
          text: `${sideLabel(event.sideId)} Reserve Phase ends: ${event.sideId === "nato" ? "Reserve" : "OMG"} markers removed from ${event.unitIds.map(name).join(", ")}`,
        });
        break;
      case "breakthroughMarkersRemoved":
        lines.push({ tone: "info", text: `${sideLabel(event.sideId)} Breakthrough markers removed from ${event.hexIds.join(", ")}` });
        break;
      case "airInterdictionZonesRemoved":
        lines.push({ tone: "info", text: `${sideLabel(event.sideId)} Air Interdiction Zones lifted (${event.hexIds.join(", ")})` });
        break;
      case "airStrikeResolved": {
        const { dieRoll, modifier, modifiedRoll, result } = event.resolution;
        lines.push({
          tone: "roll",
          text: `${event.airUnitId ? airName(event.airUnitId) : sideLabel(event.sideId)} air strike on ${event.hexId}${event.unitIds.length > 0 ? ` (${event.unitIds.map(name).join(", ")})` : ""}: rolled ${dieRoll} ${signed(modifier)} = ${modifiedRoll} → ${RESULT_LABELS[result]}`,
        });
        break;
      }
      case "airCombatRoundResolved":
        lines.push({ tone: "roll", text: `Air combat round ${event.pairing.round}: ${pairingText(event.pairing)}` });
        break;
      case "airInterceptionResolved":
        lines.push({ tone: "roll", text: `Interception: ${pairingText(event.pairing)}` });
        break;
      case "airUnitStepLost":
        lines.push({ tone: "hit", text: `${airName(event.airUnitId)} loses an air step` });
        break;
      case "airUnitAborted":
        lines.push({ tone: "hit", text: `${airName(event.airUnitId)} aborts its mission` });
        break;
      case "airUnitEliminated":
        lines.push({ tone: "hit", text: `${airName(event.airUnitId)} eliminated` });
        break;
      case "airStrikeAborted":
        lines.push({ tone: "hit", text: `${airName(event.airUnitId)} cannot complete its planned strike` });
        break;
      case "airBaseSuppressed":
        lines.push({ tone: "hit", text: `${baseName(event.airBaseId)} suppressed through turn ${event.throughTurn}` });
        break;
      case "airBaseDamaged":
        lines.push({ tone: "hit", text: `${baseName(event.airBaseId)} takes runway damage (${event.damage}/2)` });
        break;
      case "battleResolved": {
        const { report } = event;
        const attackers = report.attackingUnitIds.map(name).join(", ");
        if (!report.odds || report.dieRoll === null || !report.result) {
          lines.push({ tone: "roll", text: `${sideLabel(event.sideId)} advances into Breakthrough hex ${report.hexId} (${attackers})` });
          break;
        }
        const { odds } = report;
        const shift = odds.netShift !== 0 ? `, shift ${signed(odds.netShift)}` : "";
        const support = report.supportingHqId ? ` with Offensive Support from ${name(report.supportingHqId)}` : "";
        lines.push({
          tone: "roll",
          text: `${sideLabel(event.sideId)} attacks ${report.hexId} with ${attackers}${support}: ${odds.totalAttack} vs ${odds.totalDefense} (${ODDS_COLUMNS[odds.basicColumn]}${shift}) → ${odds.finalOdds}; rolled ${report.dieRoll}: ${report.result.code}`,
        });
        for (const roll of report.counterattacks) {
          lines.push({
            tone: "hit",
            text: `Counterattack by ${name(roll.unitId)} on ${name(roll.targetUnitId)}: rolled ${roll.dieRoll} → ${roll.disrupted ? "Disrupted" : "no effect"}`,
          });
        }
        break;
      }
      case "unitWithdrawn":
        lines.push({ tone: "info", text: `${name(event.unitId)} withdrawn from play by the scenario` });
        break;
      case "unitRetreated":
        lines.push({ tone: "hit", text: `${name(event.unitId)} retreats from ${event.from} to ${event.path[event.path.length - 1] ?? event.from}` });
        break;
      case "unitsAdvanced":
        lines.push({ tone: "info", text: `${event.unitIds.map(name).join(", ")} advance into ${event.hexId}` });
        break;
      case "cityControlChanged":
        // Only cities taken by advancing after combat belong in the Combat Log.
        if (!events.some((candidate) => candidate.type === "unitsAdvanced")) break;
        lines.push({ tone: "info", text: `${sideLabel(event.controller)} takes the city in ${event.hexId}${event.free ? " (liberated)" : ""}` });
        break;
      case "unitStepLost":
        lines.push({ tone: "hit", text: `${name(event.unitId)} loses a step` });
        break;
      case "unitEliminated":
        lines.push({ tone: "hit", text: `${name(event.unitId)} eliminated in ${event.hexId}` });
        break;
      case "unitDisruptionChanged":
        if (event.disruption) {
          lines.push({ tone: "hit", text: `${name(event.unitId)} ${event.disruption === "suppressed" ? "Suppressed" : "Disrupted"}` });
        } else if (phase === "postBattle") {
          lines.push({ tone: "info", text: `Post-Battle: ${name(event.unitId)} is no longer Suppressed` });
        } else {
          lines.push({ tone: "info", text: `Recovery: ${name(event.unitId)} is no longer Disrupted` });
        }
        break;
      case "breakthroughMarkerPlaced":
        lines.push({ tone: "info", text: `${sideLabel(event.sideId)} breakthrough in ${event.hexId}` });
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
