import { useEffect, useMemo, useState } from "react";
import { listScenarios, startNewGame, type NewGameResponse, type ScenarioSummary } from "../gameApi";
import { MenuBackdrop } from "./MenuBackdrop";
import { scenarioFamilies } from "./scenarioCatalog";

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error && "message" in error) return String(error.message);
  return String(error);
}

function sideTone(sideId: string): string {
  return sideId === "nato" ? "nato" : sideId === "warsawPact" ? "pact" : "neutral";
}

type Load = { state: "loading" } | { state: "ready"; scenarios: ScenarioSummary[] } | { state: "failed"; reason: string };

/**
 * Lists every scenario registered in the core, grouped into families with
 * year variants, and starts the chosen one. ↑ ↓ change the family, ← → the
 * year, Enter starts, Escape goes back.
 */
export function ScenarioSelect({ onBack, onStarted }: {
  onBack(): void;
  onStarted(game: NewGameResponse): void;
}) {
  const [load, setLoad] = useState<Load>({ state: "loading" });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [startError, setStartError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    listScenarios().then(
      (scenarios) => {
        if (!active) return;
        setLoad({ state: "ready", scenarios });
        setSelectedId((current) => current ?? scenarioFamilies(scenarios)[0]?.variants[0].scenario.id ?? null);
      },
      (error: unknown) => {
        if (active) setLoad({ state: "failed", reason: errorMessage(error) });
      },
    );
    return () => {
      active = false;
    };
  }, []);

  const scenarios = load.state === "ready" ? load.scenarios : [];
  const families = useMemo(() => scenarioFamilies(scenarios), [scenarios]);
  const familyIndex = families.findIndex((family) => family.variants.some((variant) => variant.scenario.id === selectedId));
  const family = families[familyIndex] ?? null;
  const variant = family?.variants.find((entry) => entry.scenario.id === selectedId) ?? null;
  const selected = variant?.scenario ?? null;

  // Moving to another family keeps the chosen year when that family has it.
  const chooseFamily = (index: number) => {
    const next = families[index];
    if (!next) return;
    const sameYear = next.variants.find((entry) => entry.year === variant?.year);
    setSelectedId((sameYear ?? next.variants[0]).scenario.id);
  };

  const start = () => {
    if (!selected || starting) return;
    setStarting(true);
    setStartError(null);
    startNewGame(selected.id).then(onStarted, (error: unknown) => {
      setStarting(false);
      setStartError(errorMessage(error));
    });
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (starting) return;
      const variantIndex = family?.variants.findIndex((entry) => entry.scenario.id === selectedId) ?? -1;
      if (event.key === "Escape") onBack();
      else if (event.key === "ArrowDown" && families.length > 0) chooseFamily((familyIndex + 1) % families.length);
      else if (event.key === "ArrowUp" && families.length > 0) chooseFamily((familyIndex + families.length - 1) % families.length);
      else if (event.key === "ArrowRight" && family && variantIndex < family.variants.length - 1) setSelectedId(family.variants[variantIndex + 1].scenario.id);
      else if (event.key === "ArrowLeft" && family && variantIndex > 0) setSelectedId(family.variants[variantIndex - 1].scenario.id);
      else if (event.key === "Enter") start();
      else return;
      event.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  let group: string | null = null;
  return (
    <div className="menu-screen scenario-screen">
      <MenuBackdrop />
      <header className="scenario-header">
        <button type="button" className="menu-back" onClick={onBack} disabled={starting}>← Main menu</button>
        <div>
          <span className="title-kicker">Operational briefing</span>
          <h1>Select Scenario</h1>
        </div>
      </header>

      <div className="scenario-body">
        <ul className="scenario-list" aria-label="Scenarios">
          {load.state === "loading" && <li className="scenario-empty">Contacting headquarters…</li>}
          {load.state === "failed" && <li className="scenario-empty error">Could not load scenarios: {load.reason}</li>}
          {load.state === "ready" && families.length === 0 && <li className="scenario-empty">No scenarios are registered.</li>}
          {families.map((entry, index) => {
            const heading = entry.group !== group ? entry.group : null;
            group = entry.group;
            const years = entry.variants.flatMap((item) => (item.year ? [item.year] : []));
            return (
              <li key={entry.key}>
                {heading && <h2 className="scenario-group">{heading}</h2>}
                <button
                  type="button"
                  className={index === familyIndex ? "scenario-card selected" : "scenario-card"}
                  onClick={() => chooseFamily(index)}
                  onDoubleClick={start}
                  disabled={starting}
                >
                  <span className="scenario-index">{String(index + 1).padStart(2, "0")}</span>
                  <span className="scenario-card-text">
                    <strong>{entry.title}</strong>
                    <small>
                      {entry.variants[0].scenario.maxGameTurns} turns
                      {years.length > 0 && ` · ${years.join(" / ")}`}
                    </small>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>

        <section className="scenario-briefing" aria-live="polite">
          {family && selected ? (
            <>
              <span className="title-kicker">{family.group}</span>
              <h2>{selected.name}</h2>
              {family.blurb && <p className="scenario-blurb">{family.blurb}</p>}
              {family.variants.length > 1 && (
                <div className="scenario-years" role="radiogroup" aria-label="Year">
                  {family.variants.map((entry) => (
                    <button
                      key={entry.scenario.id}
                      type="button"
                      role="radio"
                      aria-checked={entry.scenario.id === selectedId}
                      className={entry.scenario.id === selectedId ? "selected" : undefined}
                      disabled={starting}
                      onClick={() => setSelectedId(entry.scenario.id)}
                    >
                      {entry.year}
                    </button>
                  ))}
                  <small>The year sets the order of battle and printed strengths.</small>
                </div>
              )}
              <dl>
                <div><dt>Duration</dt><dd>{selected.maxGameTurns} game turns</dd></div>
                <div><dt>Theatre map</dt><dd>{selected.mapId}</dd></div>
                <div><dt>Identifier</dt><dd><code>{selected.id}</code></dd></div>
              </dl>
              <h3>Sides</h3>
              <div className="scenario-sides">
                {selected.sides.map((side) => (
                  <span key={side.id} className={`scenario-side ${sideTone(side.id)}`}>{side.name}</span>
                ))}
              </div>
              {startError && <p className="scenario-error">Could not start the game: {startError}</p>}
              <button type="button" className="scenario-start" onClick={start} disabled={starting}>
                {starting ? "Deploying forces…" : "Start Game"}
              </button>
            </>
          ) : (
            <p className="scenario-empty">Select a scenario to read its briefing.</p>
          )}
        </section>
      </div>
      <footer className="menu-footer">
        <span>{scenarios.length} {scenarios.length === 1 ? "scenario" : "scenarios"} registered</span>
        <span>↑ ↓ scenario · ← → year · Enter to start · Esc to go back</span>
      </footer>
    </div>
  );
}
