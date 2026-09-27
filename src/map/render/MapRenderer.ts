import { Application, Container, CullerPlugin, extensions, Graphics } from "pixi.js";
import type { AirInterdictionZone, AirMission, BattlePlan, MovementOption, UnitState } from "../../gameApi";
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
import { counterAt, populateUnitLayer, type CounterHit } from "./unitLayer";

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
  onUnitSelect(unitId: string): void;
  /** Right-click on a destination the core listed as legal for the selected unit. */
  onMoveOrder(hexId: string): void;
  onZoom(zoom: number): void;
}

/** Authoritative movement options for the selected unit, used for hover previews. */
export interface MovementPreview {
  /** Hex the unit currently occupies, or null when it is off the map. */
  origin: string | null;
  options: Map<string, MovementOption>;
}

const MOVE_ARROW_COLOR = 0xff3b30;

/** Offensive Strike Phase state drawn on the map. All of it comes from the core. */
export interface StrikeOverlay {
  /** Hexes with strikeable enemy units; `tacticalAllowed` marks friendly/contested Airspace. */
  targets: { hexId: string; tacticalAllowed: boolean }[];
  missions: AirMission[];
  zones: AirInterdictionZone[];
  breakthroughs: string[];
}

const STRIKE_COLOR = 0xff625c;
const RESOLVED_STRIKE_COLOR = 0xf0bf58;
const INTERDICTION_COLOR = 0xb58cff;
const BREAKTHROUGH_COLOR = 0xffd23f;

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
  private readonly planningLayer = new Graphics({ label: "battle-plan" });
  private readonly unitLayer = new Container({ label: "units" });
  private readonly moveArrow = new Graphics({ label: "move-preview" });
  private readonly strikeZones = new Graphics({ label: "air-interdiction" });
  private readonly strikeMarks = new Graphics({ label: "air-strikes" });
  private movePreview: MovementPreview | null = null;
  private counterHits: CounterHit[] = [];
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
      this.planningLayer,
      this.strikeZones,
      this.unitLayer,
      this.strikeMarks,
      this.highlight,
      this.moveArrow,
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
    let rightDown: { id: number; x: number; y: number } | null = null;

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
      if (e.button === 2) {
        const p = local(e);
        rightDown = { id: e.pointerId, x: p.x, y: p.y };
        return;
      }
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
      if (e.button === 2 && rightDown && rightDown.id === e.pointerId) {
        const p = local(e);
        const still = Math.hypot(p.x - rightDown.x, p.y - rightDown.y) <= DRAG_THRESHOLD;
        rightDown = null;
        const hex = this.hexAtScreen(p.x, p.y);
        // Only destinations the core listed as legal can be ordered.
        if (still && hex && this.movePreview?.options.has(hex.id)) this.callbacks.onMoveOrder(hex.id);
        return;
      }
      if (!drag || drag.id !== e.pointerId) return;
      if (!drag.moved && e.button === 0) {
        const p = local(e);
        const hex = this.hexAtScreen(p.x, p.y);
        const w = this.camera.screenToWorld(p.x, p.y);
        const unitId = counterAt(this.counterHits, w.x, w.y);
        if (unitId) {
          this.select(hex);
          this.callbacks.onUnitSelect(unitId);
        } else {
          this.select(hex && hex === this.selected ? null : hex);
        }
      }
      canvas.releasePointerCapture(e.pointerId);
      canvas.style.cursor = "";
      drag = null;
    };
    const onLeave = () => this.setHovered(null);
    const onContextMenu = (e: MouseEvent) => e.preventDefault();

    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("pointerdown", onDown);
    canvas.addEventListener("pointermove", onMove);
    canvas.addEventListener("pointerup", onUp);
    canvas.addEventListener("pointerleave", onLeave);
    canvas.addEventListener("contextmenu", onContextMenu);
    const observer = new ResizeObserver(() => this.app.resize());
    observer.observe(canvas.parentElement ?? canvas);
    this.cleanup.push(() => {
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("pointerdown", onDown);
      canvas.removeEventListener("pointermove", onMove);
      canvas.removeEventListener("pointerup", onUp);
      canvas.removeEventListener("pointerleave", onLeave);
      canvas.removeEventListener("contextmenu", onContextMenu);
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
    this.redrawMoveArrow();
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

  select(hex: HexData | null, notify = true): void {
    this.selected = hex;
    this.redrawHighlight();
    if (notify) this.callbacks.onSelect(hex);
  }

  selectById(id: string, center = true, notify = true): void {
    const hex = this.hexByKey.get(id) ?? null;
    this.select(hex, notify);
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

  setUnits(units: UnitState[]): void {
    this.counterHits = populateUnitLayer(this.unitLayer, units, this.map, this.grid);
  }

  /** Sets (or clears) the legal destinations that hover arrows and right-click orders use. */
  setMovementPreview(preview: MovementPreview | null): void {
    this.movePreview = preview;
    this.redrawMoveArrow();
  }

  private hexCenter(id: string): { x: number; y: number } | null {
    const hex = this.hexByKey.get(id);
    return hex ? this.grid.center(hex.row, hex.col) : null;
  }

  private redrawMoveArrow(): void {
    const g = this.moveArrow.clear();
    const preview = this.movePreview;
    const option = this.hovered && preview?.options.get(this.hovered.id);
    if (!preview || !option || !this.hovered) {
      this.app.canvas.style.cursor = "";
      return;
    }
    this.app.canvas.style.cursor = "pointer";
    const corners = this.grid.corners(this.hovered.row, this.hovered.col, 0.92);
    g.poly(corners).stroke({ color: MOVE_ARROW_COLOR, width: 5, alpha: 0.9, join: "round" });

    const ids = preview.origin ? [preview.origin, ...option.path] : [];
    const points = ids.map((id) => this.hexCenter(id)).filter((p): p is { x: number; y: number } => p !== null);
    if (points.length < 2) return;
    const tip = points[points.length - 1];
    const before = points[points.length - 2];
    const angle = Math.atan2(tip.y - before.y, tip.x - before.x);
    const head = 34;
    // End the shaft at the arrowhead's base so the line does not poke through it.
    const base = { x: tip.x - Math.cos(angle) * head * 0.8, y: tip.y - Math.sin(angle) * head * 0.8 };
    g.moveTo(points[0].x, points[0].y);
    for (const p of points.slice(1, -1)) g.lineTo(p.x, p.y);
    g.lineTo(base.x, base.y);
    g.stroke({ color: 0x05080c, width: 15, alpha: 0.55, cap: "round", join: "round" });
    g.moveTo(points[0].x, points[0].y);
    for (const p of points.slice(1, -1)) g.lineTo(p.x, p.y);
    g.lineTo(base.x, base.y);
    g.stroke({ color: MOVE_ARROW_COLOR, width: 9, cap: "round", join: "round" });
    const spread = 0.5;
    g.poly([
      tip.x,
      tip.y,
      tip.x - Math.cos(angle - spread) * head,
      tip.y - Math.sin(angle - spread) * head,
      tip.x - Math.cos(angle + spread) * head,
      tip.y - Math.sin(angle + spread) * head,
    ])
      .fill(MOVE_ARROW_COLOR)
      .stroke({ color: 0x05080c, width: 2, alpha: 0.7 });
  }

  setBattlePlan(plan: BattlePlan | null): void {
    const g = this.planningLayer.clear();
    if (!plan) return;
    for (const movement of plan.movements) {
      const points = movement.path.flatMap((id) => {
        const hex = this.hexByKey.get(id);
        if (!hex) return [];
        const center = this.grid.center(hex.row, hex.col);
        return [center.x, center.y];
      });
      const fromId = movement.from.type === "hex" ? movement.from.hexId : null;
      const fromHex = fromId ? this.hexByKey.get(fromId) : null;
      if (fromHex) {
        const center = this.grid.center(fromHex.row, fromHex.col);
        points.unshift(center.x, center.y);
      }
      if (points.length >= 4) {
        g.moveTo(points[0], points[1]);
        for (let i = 2; i < points.length; i += 2) g.lineTo(points[i], points[i + 1]);
        g.stroke({ color: movement.mode === "airTransport" ? 0x79cfff : 0xf0bf58, width: 7, alpha: 0.78 });
      }
    }
    for (const id of plan.attackTargets) {
      const hex = this.hexByKey.get(id);
      if (!hex) continue;
      const corners = this.grid.corners(hex.row, hex.col, 0.9);
      g.poly(corners).fill({ color: 0xd94b46, alpha: 0.2 }).stroke({ color: 0xff625c, width: 9, alpha: 0.95 });
    }
  }

  setStrikeOverlay(overlay: StrikeOverlay): void {
    const zones = this.strikeZones.clear();
    const marks = this.strikeMarks.clear();

    // An Air Interdiction Zone is the marked hex and its six neighbours.
    const zoneHexes = (centerId: string) => {
      const hex = this.hexByKey.get(centerId);
      if (!hex) return [];
      const { row, col } = hex;
      const diagonal = row % 2 === 0
        ? [[row - 1, col], [row - 1, col + 1], [row + 1, col], [row + 1, col + 1]]
        : [[row - 1, col - 1], [row - 1, col], [row + 1, col - 1], [row + 1, col]];
      return [[row, col], [row, col - 1], [row, col + 1], ...diagonal]
        .map(([r, c]) => this.hexByKey.get(hexId(r, c)))
        .filter((candidate): candidate is HexData => Boolean(candidate));
    };
    const drawZone = (hexId: string, alpha: number) => {
      for (const hex of zoneHexes(hexId)) {
        zones.poly(this.grid.corners(hex.row, hex.col)).fill({ color: INTERDICTION_COLOR, alpha });
      }
      const center = this.hexByKey.get(hexId);
      if (center) {
        zones.poly(this.grid.corners(center.row, center.col, 0.8)).stroke({ color: INTERDICTION_COLOR, width: 6, alpha: 0.9 });
      }
    };
    for (const zone of overlay.zones) drawZone(zone.hexId, 0.2);
    for (const mission of overlay.missions) {
      if (mission.kind.type === "interdiction" && !mission.resolution) drawZone(mission.hexId, 0.1);
    }

    for (const target of overlay.targets) {
      const hex = this.hexByKey.get(target.hexId);
      if (!hex) continue;
      marks.poly(this.grid.corners(hex.row, hex.col, 0.94)).stroke({
        color: target.tacticalAllowed ? STRIKE_COLOR : 0x9aa4ae,
        width: 4,
        alpha: 0.6,
      });
    }

    // Crosshair marker in the hex's upper-left corner, one per committed strike.
    const strikesByHex = new Map<string, AirMission[]>();
    for (const mission of overlay.missions) {
      if (mission.kind.type !== "strike") continue;
      strikesByHex.set(mission.hexId, [...(strikesByHex.get(mission.hexId) ?? []), mission]);
    }
    for (const [hexId, missions] of strikesByHex) {
      const hex = this.hexByKey.get(hexId);
      if (!hex) continue;
      const center = this.grid.center(hex.row, hex.col);
      missions.forEach((mission, index) => {
        const x = center.x - 40 + index * 30;
        const y = center.y - 44;
        const color = mission.resolution ? RESOLVED_STRIKE_COLOR : STRIKE_COLOR;
        marks.circle(x, y, 16).fill({ color: 0x05080c, alpha: 0.75 }).stroke({ color, width: 4 });
        marks.moveTo(x - 22, y).lineTo(x + 22, y).moveTo(x, y - 22).lineTo(x, y + 22).stroke({ color, width: 3 });
      });
    }

    for (const hexId of overlay.breakthroughs) {
      const hex = this.hexByKey.get(hexId);
      if (!hex) continue;
      const { x, y } = this.grid.center(hex.row, hex.col);
      const points: number[] = [];
      for (let i = 0; i < 10; i++) {
        const radius = i % 2 === 0 ? 30 : 13;
        const angle = -Math.PI / 2 + (i * Math.PI) / 5;
        points.push(x + Math.cos(angle) * radius, y + Math.sin(angle) * radius);
      }
      marks.poly(points).fill(BREAKTHROUGH_COLOR).stroke({ color: 0x05080c, width: 3 });
    }
  }

  focusUnits(units: UnitState[]): void {
    const points = units.flatMap((unit) => {
      if (unit.location.type !== "hex") return [];
      const hex = this.hexByKey.get(unit.location.hexId);
      return hex ? [this.grid.center(hex.row, hex.col)] : [];
    });
    if (points.length === 0) return;
    const xs = points.map((point) => point.x);
    const ys = points.map((point) => point.y);
    const margin = 180;
    const minX = Math.min(...xs) - margin;
    const maxX = Math.max(...xs) + margin;
    const minY = Math.min(...ys) - margin;
    const maxY = Math.max(...ys) + margin;
    this.camera.fitBounds({ x: minX, y: minY, width: maxX - minX, height: maxY - minY });
  }

  destroy(): void {
    for (const fn of this.cleanup) fn();
    this.app.destroy({ removeView: true }, { children: true });
  }
}
