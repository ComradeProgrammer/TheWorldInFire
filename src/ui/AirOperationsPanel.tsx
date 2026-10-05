import { useEffect, useMemo, useState } from "react";
import type {
  AirBaseState,
  AirMissionAssignment,
  AirMissionOptionsResponse,
  AirOperationsReport,
  AirPlan,
  AirPlanningOptionsResponse,
  AirUnitKind,
  AirUnitState,
  GameCommand,
  UnitState,
} from "../gameApi";
import type { HexData } from "../map/mapTypes";
import { readableId, sideName } from "./unitFormat";

const KIND_LABELS: Record<AirUnitKind, string> = {
  fighter: "Fighter",
  fighterBomber: "Fighter-bomber",
  aew: "AEW",
};

const KIND_MARKS: Record<AirUnitKind, string> = {
  fighter: "F",
  fighterBomber: "FB",
  aew: "AEW",
};

const RESULT_LABELS = {
  noEffect: "No effect",
  abort: "Abort",
  damagedAbort: "Step loss + abort",
  destroyedAbort: "Destroyed + abort",
  destroyedAndDamagedAbort: "Destroyed + damage + abort",
} as const;

function effectiveCapacity(base: AirBaseState, gameTurn: number): number {
  if (base.damage >= 2 || (base.suppressedThroughTurn !== null && gameTurn <= base.suppressedThroughTurn)) return 0;
  return base.damage === 1 ? Math.ceil(base.sortieCapacity / 2) : base.sortieCapacity;
}

function unitSteps(unit: UnitState): number {
  return unit.steps.length - unit.strengthStepIndex;
}

function isHeadquarters(unit: UnitState): boolean {
  return unit.unitTypeId === "headquarters" || unit.traits.includes("headquarters");
}

function missionTarget(mission: AirMissionAssignment, bases: AirBaseState[]): string {
  switch (mission.type) {
    case "airSuperiority": return `Air superiority · ${mission.centerHexId}`;
    case "earlyWarning": return `AEW orbit · ${mission.centerHexId}`;
    case "groundStrike": return `Ground strike · ${mission.hexId}`;
    case "airBaseStrike": return `Airbase strike · ${bases.find((base) => base.id === mission.airBaseId)?.name ?? mission.airBaseId}`;
  }
}

function missionHex(mission: AirMissionAssignment, bases: AirBaseState[]): string | null {
  if (mission.type === "airBaseStrike") {
    const baseId = mission.airBaseId;
    return bases.find((base) => base.id === baseId)?.anchorHexId ?? null;
  }
  return mission.type === "groundStrike" ? mission.hexId : mission.centerHexId;
}

function AirCounter({ unit }: { unit: AirUnitState }) {
  return (
    <span className={`air-counter side-${unit.sideId === "nato" ? "nato" : "pact"}`} aria-hidden="true">
      <b>{KIND_MARKS[unit.kind]}</b>
      <i>{unit.strengthStepIndex === 0 ? "●●" : "●○"}</i>
    </span>
  );
}

function AirUnitValues({ unit }: { unit: AirUnitState }) {
  const step = unit.steps[unit.strengthStepIndex];
  if (unit.kind === "fighter") return <span>AC {step.airCombat} · EV {step.evasion} · R {step.combatRadius}</span>;
  if (unit.kind === "fighterBomber") return <span>AC {step.airCombat} · EV {step.evasion} · Strike {step.strikeModifier >= 0 ? "+" : ""}{step.strikeModifier}</span>;
  return <span>EV {step.evasion} · AEW +{step.aewModifier} / R {step.aewRadius}</span>;
}

function PairingSummary({ report, units }: { report: AirOperationsReport; units: AirUnitState[] }) {
  const name = (id: string) => units.find((unit) => unit.id === id)?.name ?? id;
  const rows = [
    ...report.fighterCombat.map((pairing) => ({ type: `Fighter round ${pairing.round}`, pairing })),
    ...report.interceptions.map((pairing) => ({ type: "Interception", pairing })),
  ];
  if (rows.length === 0) return <p className="empty">No aircraft engaged this turn.</p>;
  return (
    <div className="air-report-list">
      {rows.map(({ type, pairing }, index) => (
        <div className="air-report-row" key={`${type}-${index}-${pairing.first.attackerId}`}>
          <strong>{type}</strong>
          {[pairing.first, pairing.second].map((attack) => (
            <span key={attack.attackerId}>
              {name(attack.attackerId)}: d20 {attack.dieRoll}{attack.modifier ? ` ${attack.modifier > 0 ? "+" : ""}${attack.modifier}` : ""} · C{attack.column >= 0 ? "+" : ""}{attack.column} → {RESULT_LABELS[attack.result]}
            </span>
          ))}
        </div>
      ))}
      {report.roundLimitReached && <p className="air-warning">Combat ended at the round limit.</p>}
    </div>
  );
}

export interface AirOperationsContext {
  gameTurn: number;
  phaseId?: string;
  activeSideId?: string;
  units: AirUnitState[];
  knownUnits: AirUnitState[];
  bases: AirBaseState[];
  plans: AirPlan[];
  report: AirOperationsReport | null;
  planningOptions: AirPlanningOptionsResponse | null;
  missionOptions: AirMissionOptionsResponse | null;
  selectedAirUnitId: string | null;
  selectedHex: HexData | null;
  queryBusy: boolean;
  commandBusy: boolean;
  groundUnits: UnitState[];
  selectedBaseId: string;
  focusedAirUnitId: string;
  onSelectAirUnit(id: string | null): void;
  onCommand(command: GameCommand): void;
  onGoTo(hexId: string): void;
  onClose(): void;
}

export function AirOperationsPanel({ context }: { context: AirOperationsContext }) {
  const {
    gameTurn, phaseId, activeSideId, units, knownUnits, bases, plans, report, planningOptions,
    missionOptions, selectedAirUnitId, selectedHex, queryBusy, commandBusy, groundUnits,
    selectedBaseId, focusedAirUnitId, onSelectAirUnit, onCommand, onGoTo, onClose,
  } = context;
  const planning = phaseId === "battlePlanning" && Boolean(activeSideId);
  const selectedBase = bases.find((base) => base.id === selectedBaseId) ?? null;
  const canCommand = planning && selectedBase?.sideId === activeSideId;
  const currentPlan = plans.find((plan) => plan.gameTurn === gameTurn && plan.sideId === activeSideId);
  const sorties = plans.filter((plan) => plan.gameTurn === gameTurn).flatMap((plan) => plan.sorties);
  const assigned = useMemo(() => new Map(sorties.map((sortie) => [sortie.airUnitId, sortie])), [sorties]);
  const unavailable = useMemo(
    () => new Map(planningOptions?.units.map((option) => [option.airUnitId, option.unavailable]) ?? []),
    [planningOptions],
  );
  const shownUnits = units.filter((unit) => unit.baseId === selectedBaseId && unit.id === focusedAirUnitId);
  const shownBases = selectedBase ? [selectedBase] : [];
  const selectedUnit = shownUnits.find((unit) => unit.id === selectedAirUnitId) ?? null;
  const [groundTargetIds, setGroundTargetIds] = useState<string[]>([]);

  useEffect(() => setGroundTargetIds([]), [selectedAirUnitId, selectedHex?.id]);

  const groundTarget = selectedHex
    ? missionOptions?.groundTargets.find((target) => target.hexId === selectedHex.id)
    : undefined;
  const targetUnits = groundTarget?.unitIds.flatMap((id) => {
    const unit = groundUnits.find((candidate) => candidate.id === id);
    return unit ? [unit] : [];
  }) ?? [];
  const alreadyTargeted = new Set(sorties.flatMap((sortie) =>
    sortie.mission.type === "groundStrike" ? sortie.mission.unitIds : [],
  ));
  const selectedSteps = groundTargetIds.reduce((sum, id) => {
    const unit = groundUnits.find((candidate) => candidate.id === id);
    return sum + (unit ? unitSteps(unit) : 0);
  }, 0);

  const toggleGroundTarget = (unit: UnitState) => {
    if (groundTargetIds.includes(unit.id)) {
      setGroundTargetIds((ids) => ids.filter((id) => id !== unit.id));
      return;
    }
    if (isHeadquarters(unit)) {
      setGroundTargetIds([unit.id]);
      return;
    }
    const withoutHeadquarters = groundTargetIds.filter((id) => {
      const target = groundUnits.find((candidate) => candidate.id === id);
      return target && !isHeadquarters(target);
    });
    if (withoutHeadquarters.reduce((sum, id) => sum + unitSteps(groundUnits.find((candidate) => candidate.id === id)!), 0) + unitSteps(unit) <= 2) {
      setGroundTargetIds([...withoutHeadquarters, unit.id]);
    }
  };

  const planAtSelectedHex = (type: "airSuperiority" | "earlyWarning") => {
    if (!selectedUnit || !selectedHex) return;
    const mission: AirMissionAssignment = type === "airSuperiority"
      ? { type, centerHexId: selectedHex.id }
      : { type, centerHexId: selectedHex.id };
    onCommand({ type: "planAirSortie", airUnitId: selectedUnit.id, mission });
  };

  return (
    <section className="air-operations-panel">
      <header className="air-operations-heading">
        <div>
          <span className="detail-eyebrow">Air command</span>
          <h3>{selectedBase?.name ?? "Air Operations"}</h3>
        </div>
        <div className="air-heading-actions">
          <span>{canCommand ? `${currentPlan?.sorties.length ?? 0} planned` : `Turn ${gameTurn}`}</span>
          <button type="button" onClick={onClose} aria-label="Close Air Command">×</button>
        </div>
      </header>

      <div className="air-base-strip">
        {shownBases.map((base) => {
          const used = sorties.filter((sortie) => knownUnits.find((unit) => unit.id === sortie.airUnitId)?.baseId === base.id).length;
          const capacity = effectiveCapacity(base, gameTurn);
          return (
            <button type="button" key={base.id} className={base.damage >= 2 ? "air-base closed" : "air-base"} onClick={() => onGoTo(base.anchorHexId)}>
              <strong>{base.name}</strong>
              <span>{used}/{capacity} sorties · damage {base.damage}/2</span>
              {base.suppressedThroughTurn !== null && gameTurn <= base.suppressedThroughTurn && <em>Suppressed through turn {base.suppressedThroughTurn}</em>}
            </button>
          );
        })}
      </div>

      <div className="air-unit-list">
        {shownUnits.map((unit) => {
          const sortie = assigned.get(unit.id);
          const reason = unavailable.get(unit.id);
          const selected = unit.id === selectedAirUnitId;
          return (
            <div className={`air-unit-card side-${unit.sideId === "nato" ? "nato" : "pact"}${selected ? " selected" : ""}`} key={unit.id}>
              <button
                type="button"
                className="air-unit-main"
                disabled={!canCommand || !planningOptions || Boolean(sortie) || Boolean(reason) || commandBusy}
                title={reason?.message}
                onClick={() => onSelectAirUnit(selected ? null : unit.id)}
              >
                <AirCounter unit={unit} />
                <span className="air-unit-copy">
                  <strong>{unit.name}</strong>
                  <small>{KIND_LABELS[unit.kind]} · <AirUnitValues unit={unit} /></small>
                </span>
                <em className={`air-readiness ${unit.readiness}`}>{sortie ? sortie.status : readableId(unit.readiness)}</em>
              </button>
              {sortie && (
                <div className="air-sortie-order">
                  <button type="button" onClick={() => { const hexId = missionHex(sortie.mission, bases); if (hexId) onGoTo(hexId); }}>
                    {missionTarget(sortie.mission, bases)}
                  </button>
                  {canCommand && currentPlan?.sideId === unit.sideId && sortie.status === "planned" && (
                    <button type="button" className="air-cancel" disabled={commandBusy} onClick={() => onCommand({ type: "cancelAirSortie", sortieId: sortie.id })}>Cancel</button>
                  )}
                </div>
              )}
              {!sortie && reason && <p className="air-unit-reason">{reason.message}</p>}
            </div>
          );
        })}
      </div>

      {canCommand && selectedUnit && (
        <div className="air-mission-editor">
          <div className="air-editor-title">
            <strong>Assign {selectedUnit.name}</strong>
            <button type="button" onClick={() => onSelectAirUnit(null)}>×</button>
          </div>
          {queryBusy || !missionOptions ? <p className="empty">Checking legal missions…</p> : (
            <>
              {(selectedUnit.kind === "fighter" || selectedUnit.kind === "aew") && (
                <div className="air-editor-section">
                  <h4>{selectedUnit.kind === "fighter" ? "Air-superiority center" : "AEW orbit center"}</h4>
                  <p>{selectedHex ? `Selected hex ${selectedHex.id}` : "Select a center hex on the map."}</p>
                  <button
                    type="button"
                    className="air-plan-button"
                    disabled={commandBusy || !selectedHex || !missionOptions.centerHexes.includes(selectedHex.id)}
                    onClick={() => planAtSelectedHex(selectedUnit.kind === "fighter" ? "airSuperiority" : "earlyWarning")}
                  >
                    Assign to {selectedHex?.id ?? "selected hex"}
                  </button>
                </div>
              )}

              {selectedUnit.kind === "fighterBomber" && (
                <>
                  <div className="air-editor-section">
                    <h4>Ground strike</h4>
                    {!selectedHex && <p>Select a target hex on the map.</p>}
                    {selectedHex && !groundTarget && <p>No legal enemy target in hex {selectedHex.id}.</p>}
                    {targetUnits.map((unit) => {
                      const checked = groundTargetIds.includes(unit.id);
                      const wasTargeted = alreadyTargeted.has(unit.id);
                      const wouldExceed = !checked && !isHeadquarters(unit) && selectedSteps + unitSteps(unit) > 2;
                      return (
                        <label className="air-ground-target" key={unit.id}>
                          <input type="checkbox" checked={checked} disabled={wasTargeted || wouldExceed} onChange={() => toggleGroundTarget(unit)} />
                          <span>{unit.name}</span>
                          <em>{wasTargeted ? "already targeted" : `${unitSteps(unit)} step${unitSteps(unit) === 1 ? "" : "s"}${isHeadquarters(unit) ? " · HQ" : ""}`}</em>
                        </label>
                      );
                    })}
                    <button
                      type="button"
                      className="air-plan-button"
                      disabled={commandBusy || !selectedHex || groundTargetIds.length === 0 || selectedSteps > 2}
                      onClick={() => selectedHex && onCommand({ type: "planAirSortie", airUnitId: selectedUnit.id, mission: { type: "groundStrike", hexId: selectedHex.id, unitIds: groundTargetIds } })}
                    >
                      Strike {selectedHex?.id ?? "selected hex"} ({selectedSteps}/2 steps)
                    </button>
                  </div>
                  <div className="air-editor-section">
                    <h4>Enemy airbase strike</h4>
                    {missionOptions.airBaseIds.map((baseId) => {
                      const base = bases.find((candidate) => candidate.id === baseId);
                      return (
                        <button
                          type="button"
                          className="air-base-target"
                          key={baseId}
                          disabled={commandBusy}
                          onClick={() => onCommand({ type: "planAirSortie", airUnitId: selectedUnit.id, mission: { type: "airBaseStrike", airBaseId: baseId } })}
                        >
                          <span>{base?.name ?? baseId}</span>
                          <em>damage {base?.damage ?? 0}/2 · anchor {base?.anchorHexId ?? "—"}</em>
                        </button>
                      );
                    })}
                  </div>
                </>
              )}
            </>
          )}
        </div>
      )}

      {!canCommand && report && (
        <div className="air-operation-report">
          <h4>Joint Air Operations report</h4>
          <PairingSummary report={report} units={knownUnits} />
        </div>
      )}
      {!canCommand && activeSideId && (
        <p className="air-phase-note">{sideName(activeSideId)} strike missions resolve automatically when this phase opens.</p>
      )}
    </section>
  );
}
