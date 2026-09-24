import type { Container } from "pixi.js";

export interface Size {
  width: number;
  height: number;
}

export interface Rect extends Size {
  x: number;
  y: number;
}

/** Pan/zoom transform applied to the world container. Purely local view state. */
export class Camera {
  zoom = 1;
  x = 0;
  y = 0;
  minZoom = 0.05;
  readonly maxZoom = 3;

  constructor(
    private readonly world: Container,
    private readonly viewport: () => Size,
    private readonly bounds: Rect,
    private readonly onChange: () => void,
  ) {}

  private apply(): void {
    this.clamp();
    this.world.scale.set(this.zoom);
    this.world.position.set(this.x, this.y);
    this.onChange();
  }

  /** Keep at least part of the map on screen. */
  private clamp(): void {
    const { width, height } = this.viewport();
    const left = this.bounds.x * this.zoom;
    const top = this.bounds.y * this.zoom;
    const w = this.bounds.width * this.zoom;
    const h = this.bounds.height * this.zoom;
    const margin = 0.35;
    this.x = Math.min(width * (1 - margin) - left, Math.max(width * margin - w - left, this.x));
    this.y = Math.min(height * (1 - margin) - top, Math.max(height * margin - h - top, this.y));
  }

  fit(padding = 24): void {
    const { width, height } = this.viewport();
    if (width <= 0 || height <= 0) return;
    const zoom = Math.min((width - padding * 2) / this.bounds.width, (height - padding * 2) / this.bounds.height);
    this.minZoom = Math.min(zoom * 0.7, 0.2);
    this.zoom = zoom;
    this.x = (width - this.bounds.width * zoom) / 2 - this.bounds.x * zoom;
    this.y = (height - this.bounds.height * zoom) / 2 - this.bounds.y * zoom;
    this.apply();
  }

  zoomAt(screenX: number, screenY: number, factor: number): void {
    const next = Math.min(this.maxZoom, Math.max(this.minZoom, this.zoom * factor));
    const wx = (screenX - this.x) / this.zoom;
    const wy = (screenY - this.y) / this.zoom;
    this.zoom = next;
    this.x = screenX - wx * next;
    this.y = screenY - wy * next;
    this.apply();
  }

  zoomCentered(factor: number): void {
    const { width, height } = this.viewport();
    this.zoomAt(width / 2, height / 2, factor);
  }

  panBy(dx: number, dy: number): void {
    this.x += dx;
    this.y += dy;
    this.apply();
  }

  centerOn(worldX: number, worldY: number, zoom = Math.max(this.zoom, 0.8)): void {
    const { width, height } = this.viewport();
    this.zoom = Math.min(this.maxZoom, Math.max(this.minZoom, zoom));
    this.x = width / 2 - worldX * this.zoom;
    this.y = height / 2 - worldY * this.zoom;
    this.apply();
  }

  screenToWorld(sx: number, sy: number): { x: number; y: number } {
    return { x: (sx - this.x) / this.zoom, y: (sy - this.y) / this.zoom };
  }
}
