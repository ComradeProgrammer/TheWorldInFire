import type { HudState } from "./hudPreview";

export interface TopBarProps {
  hud: HudState;
}

export function TopBar({ hud }: TopBarProps) {
  return (
    <header className="top-bar">
      <div className="phase-track" aria-label="Complete turn sequence">
        <span className="phase-round">
          Round {hud.turn}
          <span className="hud-dim"> / {hud.lastTurn}</span>
        </span>
        <ol className="phase-steps">
          {hud.steps.map((step) => (
            <li
              key={step.id}
              className={`${step.state} phase-side-${step.side}`}
              aria-current={step.state === "current" ? "step" : undefined}
              title={`${step.side === "joint" ? "Joint" : step.side === "nato" ? "NATO" : "Warsaw Pact"} · ${step.label}`}
            >
              {step.state === "done" && <span aria-hidden="true">✓ </span>}
              {step.label}
            </li>
          ))}
        </ol>
      </div>
    </header>
  );
}
