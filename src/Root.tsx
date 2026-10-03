import { useCallback, useState } from "react";
import App from "./App";
import { quitApp, type NewGameResponse } from "./gameApi";
import { ScenarioSelect } from "./menu/ScenarioSelect";
import { TitleScreen, type TitleAction } from "./menu/TitleScreen";
import "./menu/menu.css";

type Screen = { kind: "title" } | { kind: "scenarios" } | { kind: "game"; game: NewGameResponse };

/** Top-level screen flow: title menu → scenario selection → game. */
export function Root() {
  const [screen, setScreen] = useState<Screen>({ kind: "title" });

  const onTitleAction = useCallback((action: TitleAction) => {
    switch (action) {
      case "newGame":
        setScreen({ kind: "scenarios" });
        break;
      case "quit":
        // Outside the desktop shell (the plain Vite page) there is no app to close.
        quitApp().catch(() => window.close());
        break;
      // Load Game and Settings are placeholders for now.
      case "loadGame":
      case "settings":
        break;
    }
  }, []);

  if (screen.kind === "game") return <App key={screen.game.snapshot.gameId} game={screen.game} />;
  if (screen.kind === "scenarios") {
    return <ScenarioSelect onBack={() => setScreen({ kind: "title" })} onStarted={(game) => setScreen({ kind: "game", game })} />;
  }
  return <TitleScreen onAction={onTitleAction} />;
}
