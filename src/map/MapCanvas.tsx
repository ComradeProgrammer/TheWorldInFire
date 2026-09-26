import { useEffect, useRef } from "react";
import { HexGrid } from "./hexGrid";
import type { HexData, MapData } from "./mapTypes";
import { MapRenderer } from "./render/MapRenderer";

export interface MapCanvasProps {
  map: MapData;
  onReady(renderer: MapRenderer): void;
  onHover(hex: HexData | null): void;
  onSelect(hex: HexData | null): void;
  onUnitSelect(unitId: string): void;
  onMoveOrder(hexId: string): void;
  onZoom(zoom: number): void;
}

/** Hosts the PixiJS map renderer inside the React layout. */
export function MapCanvas(props: MapCanvasProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  // Keep the latest callbacks without recreating the renderer.
  const propsRef = useRef(props);
  useEffect(() => {
    propsRef.current = props;
  });

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let disposed = false;
    let renderer: MapRenderer | null = null;
    const grid = new HexGrid(props.map.grid);
    MapRenderer.create(host, props.map, grid, {
      onHover: (hex) => propsRef.current.onHover(hex),
      onSelect: (hex) => propsRef.current.onSelect(hex),
      onUnitSelect: (unitId) => propsRef.current.onUnitSelect(unitId),
      onMoveOrder: (hexId) => propsRef.current.onMoveOrder(hexId),
      onZoom: (zoom) => propsRef.current.onZoom(zoom),
    }).then((r) => {
      if (disposed) {
        r.destroy();
        return;
      }
      renderer = r;
      propsRef.current.onReady(r);
    });
    return () => {
      disposed = true;
      renderer?.destroy();
    };
  }, [props.map]);

  return <div ref={hostRef} className="map-canvas" />;
}
