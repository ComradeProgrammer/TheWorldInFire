import { useEffect } from "react";
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

export interface SettingsDialogProps {
  open: boolean;
  zoom: number;
  layers: Record<MapLayerId, boolean>;
  onClose(): void;
  onToggleLayer(layer: MapLayerId, visible: boolean): void;
  onZoom(factor: number): void;
  onFit(): void;
}

export function SettingsDialog(props: SettingsDialogProps) {
  const { open, onClose } = props;
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="settings-backdrop" role="presentation" onMouseDown={props.onClose}>
      <section
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <span className="dialog-kicker">Display</span>
            <h2 id="settings-title">Map Settings</h2>
          </div>
          <button type="button" className="dialog-close" onClick={props.onClose} aria-label="Close settings">×</button>
        </header>
        <div className="settings-group">
          <h3>Camera</h3>
          <div className="settings-camera">
            <button type="button" onClick={() => props.onZoom(1 / 1.25)}>Zoom out</button>
            <strong>{Math.round(props.zoom * 100)}%</strong>
            <button type="button" onClick={() => props.onZoom(1.25)}>Zoom in</button>
            <button type="button" onClick={props.onFit}>Fit map</button>
          </div>
        </div>
        <div className="settings-group">
          <h3>Map layers</h3>
          <div className="settings-layer-list">
            {(Object.keys(LAYER_LABELS) as MapLayerId[]).map((id) => (
              <label key={id}>
                <input type="checkbox" checked={props.layers[id]} onChange={(event) => props.onToggleLayer(id, event.currentTarget.checked)} />
                <span>{LAYER_LABELS[id]}</span>
              </label>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}
