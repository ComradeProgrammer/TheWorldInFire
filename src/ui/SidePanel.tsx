import { useState, type FormEvent } from "react";
import { CITY_KIND_NAMES, hexesById, natoMap, TERRAIN_NAMES } from "../map/mapData";
import type { HexData, HexsideFeature } from "../map/mapTypes";
import type { MapLayerId } from "../map/render/MapRenderer";

const LAYER_LABELS: Record<MapLayerId, string> = {
  grid: "Hex grid",
  hexNumbers: "Hex numbers",
  labels: "Place names",
  rivers: "Rivers",
  boundaries: "National borders",
  command: "Command zones",
  deployment: "Deployment areas",
};

const HEXSIDE_NAMES: Record<HexsideFeature, string> = {
  blocked: "Blocked hexside",
  corpsBoundary: "NATO corps deployment boundary",
  frontBoundary: "WP front deployment boundary",
};

function hexsideSummary(id: string): string[] {
  const out: string[] = [];
  for (const side of natoMap.hexsides) {
    if (side.a !== id && side.b !== id) continue;
    const other = side.a === id ? side.b : side.a;
    for (const f of side.features) out.push(`${HEXSIDE_NAMES[f]} (${other})`);
  }
  return out;
}

function HexDetails({ hex }: { hex: HexData | null }) {
  if (!hex) {
    return <p className="empty">Click a hex to inspect it.</p>;
  }
  const primary = hex.city ? CITY_KIND_NAMES[hex.city.kind] : TERRAIN_NAMES[hex.terrain];
  const rows: [string, string][] = [["Primary terrain", primary]];
  if (hex.city) rows.push(["Underlying terrain", TERRAIN_NAMES[hex.terrain]]);
  if (hex.city) rows.push(["Organic defense", String(hex.city.defense)]);
  if (hex.port) rows.push(["Port capacity", String(hex.port)]);
  if (hex.coastal) rows.push(["Coastal", "Yes"]);
  if (hex.mobilization) rows.push(["Mobilization site", "Yes"]);
  if (hex.commandZone) rows.push(["Command zone", hex.commandZone]);
  const sides = hexsideSummary(hex.id);
  return (
    <div className="hex-details">
      <div className="hex-title">
        <span className="hex-id">{hex.id}</span>
        <span className="hex-name">{hex.city?.name ?? hex.town ?? ""}</span>
      </div>
      <dl>
        {rows.map(([k, v]) => (
          <div key={k} className="kv">
            <dt>{k}</dt>
            <dd>{v}</dd>
          </div>
        ))}
      </dl>
      {sides.length > 0 && (
        <ul className="hexsides">
          {sides.map((s) => (
            <li key={s}>{s}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

export interface SidePanelProps {
  selected: HexData | null;
  zoom: number;
  layers: Record<MapLayerId, boolean>;
  onToggleLayer(layer: MapLayerId, visible: boolean): void;
  onZoom(factor: number): void;
  onFit(): void;
  onGoTo(id: string): boolean;
}

export function SidePanel(props: SidePanelProps) {
  const [goTo, setGoTo] = useState("");
  const [goToError, setGoToError] = useState(false);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const id = goTo.trim().padStart(4, "0");
    const ok = hexesById.has(id) && props.onGoTo(id);
    setGoToError(!ok);
  };

  return (
    <aside className="side-panel">
      <section className="panel">
        <h2>Control Panel</h2>
        <div className="panel-block">
          <h3>Selected hex</h3>
          <HexDetails hex={props.selected} />
        </div>
        <div className="panel-block">
          <h3>View</h3>
          <div className="view-controls">
            <button type="button" onClick={() => props.onZoom(1 / 1.25)} aria-label="Zoom out">
              −
            </button>
            <span className="zoom-readout">{Math.round(props.zoom * 100)}%</span>
            <button type="button" onClick={() => props.onZoom(1.25)} aria-label="Zoom in">
              +
            </button>
            <button type="button" className="wide" onClick={props.onFit}>
              Fit map
            </button>
          </div>
          <form className="goto" onSubmit={submit}>
            <input
              value={goTo}
              onChange={(e) => {
                setGoTo(e.currentTarget.value);
                setGoToError(false);
              }}
              placeholder="Go to hex, e.g. 2417"
              inputMode="numeric"
              maxLength={4}
              className={goToError ? "error" : ""}
            />
            <button type="submit">Go</button>
          </form>
        </div>
        <div className="panel-block">
          <h3>Layers</h3>
          <div className="layer-list">
            {(Object.keys(LAYER_LABELS) as MapLayerId[]).map((id) => (
              <label key={id} className="layer-toggle">
                <input
                  type="checkbox"
                  checked={props.layers[id]}
                  onChange={(e) => props.onToggleLayer(id, e.currentTarget.checked)}
                />
                {LAYER_LABELS[id]}
              </label>
            ))}
          </div>
        </div>
      </section>
      <section className="panel combat-log">
        <h2>Combat Log</h2>
        <p className="empty">No battles resolved yet.</p>
      </section>
    </aside>
  );
}
