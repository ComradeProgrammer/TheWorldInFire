import { Application, Container, CullerPlugin, extensions, Graphics } from "pixi.js";
import type { HexGrid } from "../hexGrid";
import { hexId } from "../hexGrid";
import type { HexData, MapData } from "../mapTypes";
import { Camera, type Rect } from "./camera";
import {
  buildBlockedLayer,
  buildBoundaryLayer,
  buildCausewayLayer,
  buildCommandLayer,
  buildDeploymentLayer,
  buildGridLayer,
  buildRiverLayer,
  buildWaterLayer,
} from "./featureLayers";
import { COLORS } from "./style";
import { buildCityLayer, buildHexNumberLayer, buildLabelLayers, buildSymbolLayer } from "./symbolLayers";
import { buildTerrainLayer } from "./terrainLayer";

extensions.add(CullerPlugin);

/** Map layers the UI can toggle. */
export type MapLayerId = "grid" | "hexNumbers" | "labels" | "rivers" | "boundaries" | "command" | "deployment";

export const DEFAULT_LAYERS: Record<MapLayerId, boolean> = {
  grid: true,
  hexNumbers: true,
  labels: true,
  rivers: true,
  boundaries: true,
  command: true,
  deployment: true,
};

export interface MapRendererCallbacks {
  onHover(hex: HexData | null): void;
  onSelect(hex: HexData | null): void;
  onZoom(zoom: number): void;
}

/** Extent of the playable hexes in map pixels. */
function mapBounds(map: MapData, grid: HexGrid): Rect {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const h of map.hexes) {
    const c = grid.corners(h.row, h.col);
    for (let i = 0; i < c.length; i += 2) {
      minX = Math.min(minX, c[i]);
      maxX = Math.max(maxX, c[i]);
      minY = Math.min(minY, c[i + 1]);
      maxY = Math.max(maxY, c[i + 1]);
    }
  }
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

/** Zoom thresholds for level-of-detail switching. */
const MIN_ZOOM_HEX_NUMBERS = 0.42;
const MIN_ZOOM_MINOR_LABELS = 0.2;
const DRAG_THRESHOLD = 4;

/**
 * Owns the PixiJS application that draws the NATO map. All state here is
 * presentation-only (camera, hover, selection highlight); game rules and
 * authoritative state live in the Rust core.
 */
export class MapRenderer {
  private readonly world = new Container({ label: "world" });
  private readonly layers = {} as Record<MapLayerId, Container[]>;
  private readonly layerEnabled = { ...DEFAULT_LAYERS };
  private readonly highlight = new Graphics();
  private readonly camera: Camera;
  private hexNumbers!: Container;
  private minorLabels!: Container;
  private hovered: HexData | null = null;
  private selected: HexData | null = null;
  private readonly hexByKey = new Map<string, HexData>();
  private readonly cleanup: (() => void)[] = [];

  private constructor(
    private readonly app: Application,
    private readonly map: MapData,
    private readonly grid: HexGrid,
    private readonly callbacks: MapRendererCallbacks,
  ) {
    for (const h of map.hexes) this.hexByKey.set(h.id, h);
    this.camera = new Camera(
      this.world,
      () => ({ width: app.screen.width, height: app.screen.height }),
      mapBounds(map, grid),
      () => this.onCameraChange(),
    );
    this.buildScene();
    this.bindInput();
    this.camera.fit();
  }

  static async create(host: HTMLElement, map: MapData, grid: HexGrid, callbacks: MapRendererCallbacks) {
    const app = new Application();
    await app.init({
      background: COLORS.offMap,
      resizeTo: host,
      antialias: true,
      autoDensity: true,
      resolution: Math.min(window.devicePixelRatio || 1, 2),
      preference: "webgl",
    });
    host.appendChild(app.canvas);
    return new MapRenderer(app, map, grid, callbacks);
  }

  private buildScene(): void {
    const { map, grid } = this;
    const labels = buildLabelLayers(map);
    this.hexNumbers = buildHexNumberLayer(map, grid);
    this.minorLabels = labels.minor;

    const terrain = buildTerrainLayer(map, grid);
    const water = buildWaterLayer(map);
    const rivers = buildRiverLayer(map);
    const gridLayer = buildGridLayer(map, grid);
    const blocked = buildBlockedLayer(map, grid);
    const deployment = buildDeploymentLayer(map, grid);
    const boundaries = buildBoundaryLayer(map);
    const command = buildCommandLayer(map);
    const causeways = buildCausewayLayer(map);
    const cities = buildCityLayer(map);
    const symbols = buildSymbolLayer(map);

    this.world.addChild(
      terrain,
      water,
      rivers,
      command,
      gridLayer,
      blocked,
      boundaries,
      deployment,
      causeways,
      cities,
      symbols,
      labels.minor,
      labels.major,
      this.hexNumbers,
      this.highlight,
    );
    this.layers.grid = [gridLayer];
    this.layers.hexNumbers = [this.hexNumbers];
    this.layers.labels = [labels.major, labels.minor];
    this.layers.rivers = [rivers];
    this.layers.boundaries = [boundaries];
    this.layers.command = [command];
    this.layers.deployment = [deployment];
    this.app.stage.addChild(this.world);
  }

  private bindInput(): void {
    const canvas = this.app.canvas;
    let drag: { id: number; x: number; y: number; moved: boolean } | null = null;

    const local = (e: { clientX: number; clientY: number }) => {
      const r = canvas.getBoundingClientRect();
      return { x: e.clientX - r.left, y: e.clientY - r.top };
    };

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const p = local(e);
      // Trackpad pinch arrives as ctrl+wheel with small deltas.
      const intensity = e.ctrlKey ? 0.01 : 0.0015;
      this.camera.zoomAt(p.x, p.y, Math.exp(-e.deltaY * intensity));
    };
    const onDown = (e: PointerEvent) => {
      if (e.button !== 0 && e.button !== 1) return;
      const p = local(e);
      drag = { id: e.pointerId, x: p.x, y: p.y, moved: false };
      canvas.setPointerCapture(e.pointerId);
    };
    const onMove = (e: PointerEvent) => {
      const p = local(e);
      if (drag && drag.id === e.pointerId) {
        const dx = p.x - drag.x;
        const dy = p.y - drag.y;
        if (drag.moved || Math.hypot(dx, dy) > DRAG_THRESHOLD) {
          drag.moved = true;
          canvas.style.cursor = "grabbing";
          this.camera.panBy(dx, dy);
          drag.x = p.x;
          drag.y = p.y;
          return;
        }
      }
      this.setHovered(this.hexAtScreen(p.x, p.y));
    };
    const onUp = (e: PointerEvent) => {
      if (!drag || drag.id !== e.pointerId) return;
      if (!drag.moved && e.button === 0) {
        const p = local(e);
        const hex = this.hexAtScreen(p.x, p.y);
        this.select(hex && hex === this.selected ? null : hex);
      }
      canvas.releasePointerCapture(e.pointerId);
      canvas.style.cursor = "";
      drag = null;
    };
    const onLeave = () => this.setHovered(null);

    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("pointerdown", onDown);
    canvas.addEventListener("pointermove", onMove);
    canvas.addEventListener("pointerup", onUp);
    canvas.addEventListener("pointerleave", onLeave);
    const observer = new ResizeObserver(() => this.app.resize());
    observer.observe(canvas.parentElement ?? canvas);
    this.cleanup.push(() => {
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("pointerdown", onDown);
      canvas.removeEventListener("pointermove", onMove);
      canvas.removeEventListener("pointerup", onUp);
      canvas.removeEventListener("pointerleave", onLeave);
      observer.disconnect();
    });
  }

  private hexAtScreen(sx: number, sy: number): HexData | null {
    const w = this.camera.screenToWorld(sx, sy);
    const { row, col } = this.grid.pixelToHex(w.x, w.y);
    return this.hexByKey.get(hexId(row, col)) ?? null;
  }

  private onCameraChange(): void {
    const z = this.camera.zoom;
    this.hexNumbers.visible = this.layerEnabled.hexNumbers && z >= MIN_ZOOM_HEX_NUMBERS;
    this.minorLabels.visible = this.layerEnabled.labels && z >= MIN_ZOOM_MINOR_LABELS;
    this.callbacks.onZoom(z);
  }

  private setHovered(hex: HexData | null): void {
    if (hex === this.hovered) return;
    this.hovered = hex;
    this.redrawHighlight();
    this.callbacks.onHover(hex);
  }

  private redrawHighlight(): void {
    const g = this.highlight.clear();
    if (this.selected) {
      const c = this.grid.corners(this.selected.row, this.selected.col);
      g.poly(c).fill({ color: COLORS.selection, alpha: 0.18 });
      g.poly(c).stroke({ color: COLORS.selection, width: 6, join: "round" });
    }
    if (this.hovered && this.hovered !== this.selected) {
      g.poly(this.grid.corners(this.hovered.row, this.hovered.col)).stroke({
        color: COLORS.hover,
        width: 4,
        alpha: 0.9,
        join: "round",
      });
    }
  }

  select(hex: HexData | null): void {
    this.selected = hex;
    this.redrawHighlight();
    this.callbacks.onSelect(hex);
  }

  selectById(id: string, center = true): void {
    const hex = this.hexByKey.get(id) ?? null;
    this.select(hex);
    if (hex && center) {
      const c = this.grid.center(hex.row, hex.col);
      this.camera.centerOn(c.x, c.y);
    }
  }

  setLayerVisible(layer: MapLayerId, visible: boolean): void {
    this.layerEnabled[layer] = visible;
    for (const c of this.layers[layer]) c.visible = visible;
    this.onCameraChange();
  }

  zoomBy(factor: number): void {
    this.camera.zoomCentered(factor);
  }

  fitToView(): void {
    this.camera.fit();
  }

  destroy(): void {
    for (const fn of this.cleanup) fn();
    this.app.destroy({ removeView: true }, { children: true });
  }
}
