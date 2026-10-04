import { useEffect, useState } from "react";
import { MenuBackdrop } from "./MenuBackdrop";

export type TitleAction = "newGame" | "loadGame" | "settings" | "quit";

const ITEMS: { action: TitleAction; label: string; hint: string; soon?: boolean }[] = [
  { action: "newGame", label: "New Game", hint: "Choose a scenario and take command" },
  { action: "loadGame", label: "Load Game", hint: "Resume a saved campaign", soon: true },
  { action: "settings", label: "Settings", hint: "Display and game options", soon: true },
  { action: "quit", label: "Quit", hint: "Stand down and close the game" },
];

/** DEFCON readiness strip: level 3 lit, for flavour. */
function Defcon() {
  return (
    <div className="defcon" aria-label="DEFCON 3">
      <span>DEFCON</span>
      {[5, 4, 3, 2, 1].map((level) => (
        <b key={level} className={level === 3 ? "lit" : level > 3 ? "past" : undefined}>{level}</b>
      ))}
    </div>
  );
}

/** Main menu shown when the game starts. Arrow keys move, Enter selects. */
export function TitleScreen({ onAction }: { onAction(action: TitleAction): void }) {
  const [active, setActive] = useState(0);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "ArrowDown") setActive((index) => (index + 1) % ITEMS.length);
      else if (event.key === "ArrowUp") setActive((index) => (index + ITEMS.length - 1) % ITEMS.length);
      else if (event.key === "Enter") onAction(ITEMS[active].action);
      else return;
      event.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [active, onAction]);

  return (
    <div className="menu-screen title-screen">
      <MenuBackdrop />
      <div className="classification">EXERCISE · EXERCISE · EXERCISE</div>
      <main className="title-layout">
        <section className="title-block">
          <span className="title-kicker">Central Front · 1983</span>
          <h1 className="title-name">OOAW</h1>
          <p className="title-sub">
            Opensource
            <br />
            Operations Across Warfare
          </p>
        </section>
        <nav className="title-menu" aria-label="Main menu">
          {ITEMS.map((item, index) => (
            <button
              key={item.action}
              type="button"
              className={index === active ? "menu-item active" : "menu-item"}
              onMouseEnter={() => setActive(index)}
              onFocus={() => setActive(index)}
              onClick={() => onAction(item.action)}
            >
              <span className="menu-item-index">0{index + 1}</span>
              <span className="menu-item-text">
                <strong>{item.label}</strong>
                <small>{item.hint}</small>
              </span>
              {item.soon && <span className="menu-soon">Soon</span>}
            </button>
          ))}
        </nav>
      </main>
      <footer className="menu-footer">
        <span>OOAW · Operational prototype</span>
        <Defcon />
        <span>↑ ↓ to choose · Enter to select</span>
      </footer>
    </div>
  );
}
