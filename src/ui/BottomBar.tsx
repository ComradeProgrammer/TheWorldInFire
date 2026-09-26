import type { BattlePlan, MovementMode, UnitState } from "../gameApi";

function unitName(units: UnitState[], id: string): string {
  return units.find((unit) => unit.id === id)?.name ?? id;
}

const MODE_LABELS: Record<MovementMode, string> = {
  tactical: "Move",
  march: "March",
  rail: "Rail",
  airTransport: "Airlift",
};

function fromLabel(movement: BattlePlan["movements"][number]): string {
  return movement.from.type === "hex" ? movement.from.hexId : "Strategic Reserve";
}

export function BottomBar({ plan, units, combatLog, collapsed, onToggleCollapsed }: {
  plan: BattlePlan | null;
  units: UnitState[];
  combatLog: string[];
  collapsed: boolean;
  onToggleCollapsed(): void;
}) {
  const orderCount = plan
    ? (plan.resupplyTargetUnitId ? 1 : 0)
      + plan.attackTargets.length
      + plan.movements.length
      + plan.entrainingUnitIds.length
      + plan.detrainedUnitIds.length
    : 0;

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
          <div className="readonly-log-body">
            {combatLog.length === 0
              ? <p className="empty">No combat results yet.</p>
              : combatLog.map((entry, index) => <p key={`${index}-${entry}`}><span>{entry}</span></p>)}
          </div>
        )}
      </section>
    </footer>
  );
}
