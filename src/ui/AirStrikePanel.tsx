import { useEffect, useState } from "react";
import type { AirMission, AirPointKind, AirPoints, Airspace, GameCommand, StrikePlan, StrikeTargetHex, UnitState } from "../gameApi";
import type { HexData } from "../map/mapTypes";

const AIRSPACE_LABELS: Record<Airspace, string> = {
  friendly: "Friendly airspace",
  contested: "Contested airspace",
  enemy: "Enemy airspace",
};

const RESULT_LABELS = { noEffect: "No effect", disrupted: "Disrupted", stepLoss: "Step loss" } as const;

function signed(value: number): string {
  return value >= 0 ? `+${value}` : `${value}`;
}

function missionLabel(mission: AirMission, units: UnitState[]): string {
  const source = mission.source === "operational" ? "Operational" : mission.source === "bonusTactical" ? "Tactical (bonus)" : "Tactical";
  if (mission.kind.type === "interdiction") return `Interdiction · ${source}`;
  const names = mission.kind.unitIds.map((id) => units.find((unit) => unit.id === id)?.name ?? "Eliminated unit");
  return `Strike: ${names.join(", ")} · ${source}`;
}

export interface AirStrikePanelProps {
  hex: HexData;
  units: UnitState[];
  /** Core preview for this hex, when it holds strikeable enemy units. */
  target: StrikeTargetHex | undefined;
  /** Airspace of the hex from the striking side's view, from the core preview. */
  airspace: Airspace | null;
  /** Whether the core allows Tactical Air Points in this hex. */
  tacticalAllowed: boolean;
  plan: StrikePlan;
  points: AirPoints | undefined;
  busy: boolean;
  onCommand(command: GameCommand): void;
}

/** Air Strike Segment controls for the selected hex. The core validates every order. */
export function AirStrikePanel({ hex, units, target, airspace, tacticalAllowed, plan, points, busy, onCommand }: AirStrikePanelProps) {
  const [chosen, setChosen] = useState<string[]>([]);
  useEffect(() => setChosen([]), [hex.id, plan.missions.length]);

  const missions = plan.missions.filter((mission) => mission.hexId === hex.id);
  const tacticalLeft = (points?.tactical ?? 0) + (points?.bonusTactical ?? 0);
  const operationalLeft = points?.operational ?? 0;
  const locked = busy || plan.resolved;
  const toggle = (unitId: string) =>
    setChosen((current) => (current.includes(unitId) ? current.filter((id) => id !== unitId) : [...current, unitId]));
  const strike = (airPoint: AirPointKind) => onCommand({ type: "planAirStrike", hexId: hex.id, unitIds: chosen, airPoint });
  const interdict = (airPoint: AirPointKind) => onCommand({ type: "planAirInterdiction", hexId: hex.id, airPoint });

  return (
    <section className="air-strike-panel">
      <header>
        <h3>Air missions</h3>
        {airspace && <span className={`airspace-tag ${airspace}`}>{AIRSPACE_LABELS[airspace]}</span>}
      </header>
      <p className="air-points-line">
        Air Points left: <strong>{points?.tactical ?? 0}</strong> Tactical
        {points && points.bonusTactical > 0 && <> (+{points.bonusTactical} bonus)</>} · <strong>{operationalLeft}</strong> Operational
      </p>

      {missions.map((mission) => (
        <div key={mission.id} className="mission-row">
          <span>{missionLabel(mission, units)}</span>
          {mission.resolution ? (
            <strong className={`strike-result ${mission.resolution.result}`}>
              {mission.resolution.dieRoll} {signed(mission.resolution.modifier)} = {mission.resolution.modifiedRoll} · {RESULT_LABELS[mission.resolution.result]}
            </strong>
          ) : plan.resolved ? (
            <strong className="strike-result zone">Zone active</strong>
          ) : (
            <button type="button" className="action-toggle active" disabled={locked} onClick={() => onCommand({ type: "cancelAirMission", missionId: mission.id })}>
              Undo {mission.kind.type === "strike" ? "strike" : "interdiction"}
            </button>
          )}
        </div>
      ))}

      {plan.resolved ? (
        <p className="readonly-note">This phase's air missions have been resolved.</p>
      ) : (
        <>
          {target && target.units.length > 0 && (
            <div className="strike-targets">
              <h4>Strike targets ({target.strikesRemaining} strike{target.strikesRemaining === 1 ? "" : "s"} left here)</h4>
              {target.units.map((entry) => {
                const unit = units.find((candidate) => candidate.id === entry.unitId);
                return (
                  <label key={entry.unitId} className={entry.alreadyTargeted ? "strike-target targeted" : "strike-target"}>
                    <input type="checkbox" checked={chosen.includes(entry.unitId)} disabled={locked || entry.alreadyTargeted} onChange={() => toggle(entry.unitId)} />
                    <span>{unit?.name ?? entry.unitId}</span>
                    <small>{entry.steps} step{entry.steps === 1 ? "" : "s"}{entry.headquarters ? " · HQ" : ""}</small>
                    <strong title="Die roll modifier">{signed(entry.modifier)}</strong>
                  </label>
                );
              })}
              <p className="strike-hint">
                {chosen.length > 1 ? "The first unit checked takes any step loss." : "Check up to two steps of enemy units."}
              </p>
              <div className="strike-buttons">
                <button type="button" disabled={locked || chosen.length === 0 || !target.tacticalAllowed || tacticalLeft === 0} onClick={() => strike("tactical")}>
                  Strike · Tactical
                </button>
                <button type="button" disabled={locked || chosen.length === 0 || operationalLeft === 0} onClick={() => strike("operational")}>
                  Strike · Operational
                </button>
              </div>
            </div>
          )}
          {hex.terrain !== "sea" && (
            <div className="strike-buttons interdiction">
              <button type="button" disabled={locked || !tacticalAllowed || tacticalLeft === 0} onClick={() => interdict("tactical")}>
                Interdict · Tactical
              </button>
              <button type="button" disabled={locked || operationalLeft === 0} onClick={() => interdict("operational")}>
                Interdict · Operational
              </button>
            </div>
          )}
        </>
      )}
    </section>
  );
}
