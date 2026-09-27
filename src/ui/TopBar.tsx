import type { HudState } from "./hudPreview";

export interface TopBarProps {
  hud: HudState;
  canEndPhase: boolean;
  phaseActionLabel: string;
  phaseActionBusy: boolean;
  onOpenSettings(): void;
  onEndPhase(): void;
}

export function TopBar({ hud, canEndPhase, phaseActionLabel, phaseActionBusy, onOpenSettings, onEndPhase }: TopBarProps) {
  const sideClass = hud.activePlayer === "NATO" ? "nato" : hud.activePlayer === "Warsaw Pact" ? "pact" : "neutral";
  return (
    <header className="top-bar">
      <div className="brand">
        <span className="brand-mark">OOAW</span>
        <span className="brand-sub">NATO: The Cold War Goes Hot</span>
      </div>
      <div className={`phase-track track-${sideClass}`} aria-label="Turn progress">
        <span className={`phase-side side-${sideClass}`}>{hud.activePlayer === "Both" ? "Joint" : hud.activePlayer}</span>
        <span className="phase-round">
          Round {hud.turn}
          <span className="hud-dim"> / {hud.lastTurn}</span>
        </span>
        {hud.steps.length > 0 ? (
          <ol className="phase-steps">
            {hud.steps.map((step) => (
              <li key={step.id} className={step.state} aria-current={step.state === "current" ? "step" : undefined}>
                {step.state === "done" && <span aria-hidden="true">✓ </span>}
                {step.label}
              </li>
            ))}
          </ol>
        ) : (
          <span className="hud-value">{hud.phase}</span>
        )}
      </div>
      <div className="hud-group resources">
        {hud.resources.map((r) => (
          <div className="hud-item" key={r.label}>
            <span className="hud-label">{r.label}</span>
            <span className={`hud-value side-${r.side}`}>{r.value}</span>
          </div>
        ))}
      </div>
      <div className="scenario-tag" title="Authoritative game state from the Rust core">
        {hud.scenario}
      </div>
      <button className="settings-action" type="button" onClick={onOpenSettings} aria-label="Open display settings">
        <span aria-hidden="true">⚙</span> Settings
      </button>
      <button className="phase-action" type="button" disabled={!canEndPhase || phaseActionBusy} onClick={onEndPhase}>
        {phaseActionBusy ? "Resolving…" : phaseActionLabel}
      </button>
    </header>
  );
}
