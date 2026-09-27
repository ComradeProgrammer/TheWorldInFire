import { useEffect, useRef } from "react";
import type { AirMission, BattlePlan, MovementMode, StrikePlan, UnitState } from "../gameApi";
import type { CombatLogEntry } from "./combatLog";

function unitName(units: UnitState[], id: string): string {
  return units.find((unit) => unit.id === id)?.name ?? id;
}

const MODE_LABELS: Record<MovementMode, string> = {
  tactical: "Move",
  march: "March",
  rail: "Rail",
  airTransport: "Airlift",
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

function fromLabel(movement: BattlePlan["movements"][number]): string {
  return movement.from.type === "hex" ? movement.from.hexId : "Strategic Reserve";
}

export function BottomBar({ plan, strikePlan, units, combatLog, collapsed, onToggleCollapsed }: {
  plan: BattlePlan | null;
  strikePlan: StrikePlan | null;
  units: UnitState[];
  combatLog: CombatLogEntry[];
  collapsed: boolean;
  onToggleCollapsed(): void;
}) {
  const orderCount = plan
    ? (plan.resupplyTargetUnitId ? 1 : 0)
      + plan.attackTargets.length
      + plan.movements.length
      + plan.entrainingUnitIds.length
      + plan.detrainedUnitIds.length
      + (strikePlan?.missions.length ?? 0)
    : 0;
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
              : "No active plan"}
          </span>
        </header>
        {!collapsed && (
          <div className="readonly-log-body">
            {!plan && <p className="empty">No active battle plan.</p>}
            {plan && orderCount === 0 && <p className="empty">No orders have been added yet.</p>}
            {plan?.resupplyTargetUnitId && <p><b>Resupply</b><span>{unitName(units, plan.resupplyTargetUnitId)}</span></p>}
            {plan?.attackTargets.map((hex) => <p key={`attack-${hex}`}><b>Attack</b><span>Objective hex {hex}</span></p>)}
            {plan?.movements.map((movement, index) => (
              <p key={`${movement.unitId}-${index}`}>
                <b>{MODE_LABELS[movement.mode]}</b>
                <span>{unitName(units, movement.unitId)} · {fromLabel(movement)} → {movement.to}</span>
              </p>
            ))}
            {plan?.entrainingUnitIds.map((id) => <p key={`entrain-${id}`}><b>Entrain</b><span>{unitName(units, id)}</span></p>)}
            {plan?.detrainedUnitIds.map((id) => <p key={`detrain-${id}`}><b>Detrain</b><span>{unitName(units, id)}</span></p>)}
            {strikePlan?.missions.map((mission) => (
              <p key={`mission-${mission.id}`}>
                <b>{mission.kind.type === "strike" ? "Strike" : "Interdict"}</b>
                <span>{missionText(mission, units)}</span>
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
