import { useCallback, useRef, useState } from "react";
import { MapCanvas } from "./map/MapCanvas";
import { natoMap } from "./map/mapData";
import type { HexData } from "./map/mapTypes";
import { DEFAULT_LAYERS, type MapLayerId, type MapRenderer } from "./map/render/MapRenderer";
import { BottomBar, type LogEntry } from "./ui/BottomBar";
import { PREVIEW_HUD } from "./ui/hudPreview";
import { SidePanel } from "./ui/SidePanel";
import { TopBar } from "./ui/TopBar";
import "./App.css";

const MAX_LOG = 200;

function App() {
  const rendererRef = useRef<MapRenderer | null>(null);
  const [hovered, setHovered] = useState<HexData | null>(null);
  const [selected, setSelected] = useState<HexData | null>(null);
  const [zoom, setZoom] = useState(1);
  const [layers, setLayers] = useState(DEFAULT_LAYERS);
  const [log, setLog] = useState<LogEntry[]>([]);
  const nextLogId = useRef(0);

  const addLog = useCallback((text: string, tone?: LogEntry["tone"]) => {
    setLog((prev) => [...prev.slice(-(MAX_LOG - 1)), { id: nextLogId.current++, text, tone }]);
  }, []);

  const onReady = useCallback(
    (renderer: MapRenderer) => {
      rendererRef.current = renderer;
      const cities = natoMap.hexes.filter((h) => h.city).length;
      addLog(`Map loaded: ${natoMap.hexes.length} hexes, ${cities} city hexes.`);
    },
    [addLog],
  );

  const onSelect = useCallback(
    (hex: HexData | null) => {
      setSelected(hex);
      if (hex) addLog(`Selected hex ${hex.id}${hex.city ? ` — ${hex.city.name}` : ""}.`);
    },
    [addLog],
  );

  const toggleLayer = (layer: MapLayerId, visible: boolean) => {
    setLayers((prev) => ({ ...prev, [layer]: visible }));
    rendererRef.current?.setLayerVisible(layer, visible);
  };

  return (
    <div className="app">
      <TopBar hud={PREVIEW_HUD} />
      <main className="map-area">
        <MapCanvas onReady={onReady} onHover={setHovered} onSelect={onSelect} onZoom={setZoom} />
      </main>
      <SidePanel
        selected={selected}
        zoom={zoom}
        layers={layers}
        onToggleLayer={toggleLayer}
        onZoom={(f) => rendererRef.current?.zoomBy(f)}
        onFit={() => rendererRef.current?.fitToView()}
        onGoTo={(id) => {
          if (!rendererRef.current) return false;
          rendererRef.current.selectById(id);
          return true;
        }}
      />
      <BottomBar log={log} hovered={hovered} />
    </div>
  );
}

export default App;
