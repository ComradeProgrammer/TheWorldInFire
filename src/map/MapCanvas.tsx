import { useEffect, useRef } from "react";
import { natoGrid, natoMap } from "./mapData";
import type { HexData } from "./mapTypes";
import { MapRenderer } from "./render/MapRenderer";

export interface MapCanvasProps {
  onReady(renderer: MapRenderer): void;
  onHover(hex: HexData | null): void;
  onSelect(hex: HexData | null): void;
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
    MapRenderer.create(host, natoMap, natoGrid, {
      onHover: (hex) => propsRef.current.onHover(hex),
      onSelect: (hex) => propsRef.current.onSelect(hex),
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
  }, []);

  return <div ref={hostRef} className="map-canvas" />;
}
