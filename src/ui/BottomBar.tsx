import { useEffect, useRef } from "react";
import { CITY_KIND_NAMES, TERRAIN_NAMES } from "../map/mapData";
import type { HexData } from "../map/mapTypes";

export interface LogEntry {
  id: number;
  text: string;
  tone?: "info" | "warn";
}

function describeHex(hex: HexData): string {
  const parts = [hex.id];
  if (hex.city) parts.push(`${hex.city.name} (${CITY_KIND_NAMES[hex.city.kind]})`);
  else if (hex.town) parts.push(hex.town);
  parts.push(TERRAIN_NAMES[hex.terrain]);
  if (hex.commandZone) parts.push(hex.commandZone);
  return parts.join(" · ");
}

export function BottomBar({ log, hovered }: { log: LogEntry[]; hovered: HexData | null }) {
  const listRef = useRef<HTMLOListElement>(null);
  useEffect(() => {
    const el = listRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [log.length]);

  return (
    <footer className="bottom-bar">
      <section className="event-log">
        <h2>Event Log</h2>
        <ol ref={listRef}>
          {log.map((e) => (
            <li key={e.id} className={e.tone === "warn" ? "warn" : undefined}>
              {e.text}
            </li>
          ))}
        </ol>
      </section>
      <div className="hint-bar">
        <span className="hover-info">{hovered ? describeHex(hovered) : "—"}</span>
        <span className="controls-hint">Scroll to zoom · Drag to pan · Click to select a hex</span>
      </div>
    </footer>
  );
}
