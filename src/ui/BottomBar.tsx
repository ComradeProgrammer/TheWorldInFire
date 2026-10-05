import { useEffect, useRef } from "react";
import type { AirBaseState, AirMission, AirPlan, AirSortie, AirUnitState, BattlePlan, BattleReport, MovementMode, ReserveState, StrikePlan, UnitState } from "../gameApi";
import type { CombatLogEntry } from "./combatLog";

function unitName(units: UnitState[], id: string): string {
  return units.find((unit) => unit.id === id)?.name ?? id;
}

const MODE_LABELS: Record<MovementMode, string> = {
  tactical: "Move",
  march: "March",
  rail: "Rail",
  airTransport: "Airlift",
  paradrop: "Paradrop",
  seaTransport: "Sealift",
};

const RESULT_LABELS = { noEffect: "no effect", disrupted: "Disrupted", stepLoss: "step loss" } as const;

function missionText(mission: AirMission, units: UnitState[]): string {
  const point = mission.source === "operational" ? "Operational" : "Tactical";
  const target = mission.kind.type === "interdiction"
    ? `Interdiction zone at ${mission.hexId}`
    : `${mission.hexId} · ${mission.kind.unitIds.map((id) => unitName(units, id)).join(", ")}`;
  const result = mission.resolution ? ` · ${RESULT_LABELS[mission.resolution.result]}` : "";
  return `${target} · ${point}${result}`;
}

function sortieText(sortie: AirSortie, airUnits: AirUnitState[], bases: AirBaseState[], units: UnitState[]): string {
  const aircraft = airUnits.find((unit) => unit.id === sortie.airUnitId)?.name ?? sortie.airUnitId;
  let target: string;
  switch (sortie.mission.type) {
    case "airSuperiority": target = `air superiority at ${sortie.mission.centerHexId}`; break;
    case "earlyWarning": target = `AEW orbit at ${sortie.mission.centerHexId}`; break;
    case "groundStrike": target = `strike ${sortie.mission.hexId} · ${sortie.mission.unitIds.map((id) => unitName(units, id)).join(", ")}`; break;
    case "airBaseStrike": {
      const baseId = sortie.mission.airBaseId;
      target = `strike ${bases.find((base) => base.id === baseId)?.name ?? baseId}`;
      break;
    }
  }
  return `${aircraft} · ${target} · ${sortie.status}`;
}

function fromLabel(movement: BattlePlan["movements"][number]): string {
  return movement.from.type === "hex" ? movement.from.hexId : "Strategic Reserve";
}

function battleText(battle: BattleReport, units: UnitState[]): string {
  const attackers = battle.attackingUnitIds.map((id) => unitName(units, id)).join(", ");
  if (!battle.result || battle.dieRoll === null) return `${battle.hexId} · Breakthrough advance · ${attackers}`;
  return `${battle.hexId} · ${battle.odds?.finalOdds ?? ""} · rolled ${battle.dieRoll} → ${battle.result.code} · ${attackers}`;
}

export function BottomBar({ plan, strikePlan, airPlans, airUnits, airBases, battles, reserve, units, combatLog, collapsed, onToggleCollapsed }: {
  plan: BattlePlan | null;
  strikePlan: StrikePlan | null;
  airPlans: AirPlan[];
  airUnits: AirUnitState[];
  airBases: AirBaseState[];
  battles: BattleReport[];
  /** Reserve Phase movement, while that phase is active. */
  reserve: ReserveState | null;
  units: UnitState[];
  combatLog: CombatLogEntry[];
  collapsed: boolean;
  onToggleCollapsed(): void;
}) {
  const airSorties = airPlans
    .filter((airPlan) => !plan || airPlan.sideId === plan.sideId)
    .flatMap((airPlan) => airPlan.sorties);
  const orderCount = (plan
    ? plan.resupplyTargetUnitIds.length
      + plan.attackTargets.length
      + plan.movements.length
      + plan.entrainingUnitIds.length
      + plan.detrainedUnitIds.length
      + plan.reserveUnitIds.length
      + (strikePlan?.missions.length ?? 0)
      + battles.length
      + (reserve?.movements.length ?? 0)
    : 0) + airSorties.length;
  const reserveLabel = plan?.sideId === "nato" ? "Reserve" : "OMG";
  const logRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const body = logRef.current;
    if (body) body.scrollTop = body.scrollHeight;
  }, [combatLog.length, collapsed]);

  return (
    <footer className={collapsed ? "bottom-bar collapsed" : "bottom-bar"}>
      <section className="readonly-log battle-plan-log" aria-label="Battle Plan">
        <header>
          <h2>Battle Plan</h2>
          <span>
            {plan
              ? `${plan.sideId === "nato" ? "NATO" : "Warsaw Pact"} · Turn ${plan.gameTurn} · ${orderCount} ${orderCount === 1 ? "order" : "orders"}`
              : airSorties.length > 0 ? `${airSorties.length} air ${airSorties.length === 1 ? "sortie" : "sorties"}` : "No active plan"}
          </span>
        </header>
        {!collapsed && (
          <div className="readonly-log-body">
            {!plan && airSorties.length === 0 && <p className="empty">No active battle plan.</p>}
            {plan && orderCount === 0 && <p className="empty">No orders have been added yet.</p>}
            {plan?.resupplyTargetUnitIds.map((id) => <p key={`resupply-${id}`}><b>Resupply</b><span>{unitName(units, id)}</span></p>)}
            {plan?.attackTargets.map((hex) => <p key={`attack-${hex}`}><b>Attack</b><span>Objective hex {hex}</span></p>)}
            {plan?.movements.map((movement, index) => (
              <p key={`${movement.unitId}-${index}`}>
                <b>{MODE_LABELS[movement.mode]}</b>
                <span>{unitName(units, movement.unitId)} · {fromLabel(movement)} → {movement.to}</span>
              </p>
            ))}
            {plan?.entrainingUnitIds.map((id) => <p key={`entrain-${id}`}><b>Entrain</b><span>{unitName(units, id)}</span></p>)}
            {plan?.detrainedUnitIds.map((id) => <p key={`detrain-${id}`}><b>Detrain</b><span>{unitName(units, id)}</span></p>)}
            {plan?.reserveUnitIds.map((id) => <p key={`reserve-${id}`}><b>{reserveLabel}</b><span>{unitName(units, id)}</span></p>)}
            {airSorties.map((sortie) => (
              <p key={`air-sortie-${sortie.airUnitId}`}>
                <b>Air</b>
                <span>{sortieText(sortie, airUnits, airBases, units)}</span>
              </p>
            ))}
            {battles.map((battle) => (
              <p key={`battle-${battle.id}`}>
                <b>Battle</b>
                <span>{battleText(battle, units)}</span>
              </p>
            ))}
            {strikePlan?.missions.map((mission) => (
              <p key={`mission-${mission.id}`}>
                <b>{mission.kind.type === "strike" ? "Strike" : "Interdict"}</b>
                <span>{missionText(mission, units)}</span>
              </p>
            ))}
            {reserve?.movements.map((movement, index) => (
              <p key={`reserve-move-${movement.unitId}-${index}`}>
                <b>{reserveLabel} move</b>
                <span>{unitName(units, movement.unitId)} · {fromLabel(movement)} → {movement.to}</span>
              </p>
            ))}
          </div>
        )}
      </section>
      <section className="readonly-log combat-readonly-log" aria-label="Combat Log">
        <header>
          <h2>Combat Log</h2>
          <span>{combatLog.length} {combatLog.length === 1 ? "entry" : "entries"}</span>
          <button
            type="button"
            className="log-toggle"
            aria-expanded={!collapsed}
            onClick={onToggleCollapsed}
          >
            {collapsed ? "▴ Expand" : "▾ Collapse"}
          </button>
        </header>
        {!collapsed && (
          <div className="readonly-log-body" ref={logRef}>
            {combatLog.length === 0
              ? <p className="empty">No combat results yet.</p>
              : combatLog.map((entry) => <p key={entry.id} className={`log-${entry.tone}`}><span>{entry.text}</span></p>)}
          </div>
        )}
      </section>
    </footer>
  );
}
