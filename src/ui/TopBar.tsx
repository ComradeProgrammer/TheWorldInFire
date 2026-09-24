import type { HudState } from "./hudPreview";

export function TopBar({ hud }: { hud: HudState }) {
  const sideClass = hud.activePlayer === "NATO" ? "nato" : "pact";
  return (
    <header className="top-bar">
      <div className="brand">
        <span className="brand-mark">OOAW</span>
        <span className="brand-sub">NATO: The Cold War Goes Hot</span>
      </div>
      <div className="hud-group">
        <div className="hud-item">
          <span className="hud-label">Turn</span>
          <span className="hud-value">
            {hud.turn}
            <span className="hud-dim"> / {hud.lastTurn}</span>
          </span>
        </div>
        <div className="hud-item">
          <span className="hud-label">Player</span>
          <span className={`hud-value side-${sideClass}`}>{hud.activePlayer}</span>
        </div>
        <div className="hud-item">
          <span className="hud-label">Phase</span>
          <span className="hud-value">{hud.phase}</span>
        </div>
      </div>
      <div className="hud-group resources">
        {hud.resources.map((r) => (
          <div className="hud-item" key={r.label}>
            <span className="hud-label">{r.label}</span>
            <span className={`hud-value side-${r.side}`}>{r.value}</span>
          </div>
        ))}
      </div>
      <div className="scenario-tag" title="Game state will come from the Rust core">
        {hud.scenario}
      </div>
    </header>
  );
}
