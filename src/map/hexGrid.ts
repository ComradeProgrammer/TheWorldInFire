import type { MapGrid } from "./mapTypes";

/**
 * Pointy-top hex geometry matching the printed NATO map.
 *
 * Rows are horizontal and odd rows are shifted half a hex to the right.
 * Printed columns count *down* from left to right, so the zero-based grid
 * column index is `columnBase - col`.
 */
export class HexGrid {
  readonly halfWidth: number;
  /** Vertical circumradius; rows are 1.5 radii apart. */
  readonly radiusY: number;
  private readonly yStretch: number;

  constructor(readonly grid: MapGrid) {
    this.halfWidth = grid.hexWidth / 2;
    this.radiusY = grid.rowSpacing / 1.5;
    this.yStretch = grid.hexWidth / Math.sqrt(3) / this.radiusY;
  }

  center(row: number, col: number): { x: number; y: number } {
    const m = this.grid.columnBase - col;
    return {
      x: this.grid.originX + this.grid.hexWidth * m + (row % 2 ? this.halfWidth : 0),
      y: this.grid.originY + this.grid.rowSpacing * row,
    };
  }

  /** Corner coordinates, clockwise from the top vertex, as a flat list. */
  corners(row: number, col: number, scale = 1): number[] {
    const { x, y } = this.center(row, col);
    const hw = this.halfWidth * scale;
    const ry = this.radiusY * scale;
    return [x, y - ry, x + hw, y - ry / 2, x + hw, y + ry / 2, x, y + ry, x - hw, y + ry / 2, x - hw, y - ry / 2];
  }

  /** Nearest hex to a map-pixel position. */
  pixelToHex(x: number, y: number): { row: number; col: number } {
    const r0 = Math.round((y - this.grid.originY) / this.grid.rowSpacing);
    let best = { d: Infinity, row: r0, col: 0 };
    for (let row = r0 - 1; row <= r0 + 1; row++) {
      const shift = row % 2 ? this.halfWidth : 0;
      const m = Math.round((x - this.grid.originX - shift) / this.grid.hexWidth);
      for (let mm = m - 1; mm <= m + 1; mm++) {
        const col = this.grid.columnBase - mm;
        const c = this.center(row, col);
        const d = (c.x - x) ** 2 + ((c.y - y) * this.yStretch) ** 2;
        if (d < best.d) best = { d, row, col };
      }
    }
    return { row: best.row, col: best.col };
  }

  /** Endpoints of the side shared by two adjacent hexes. */
  sharedSide(a: { row: number; col: number }, b: { row: number; col: number }): number[] | null {
    const ca = this.corners(a.row, a.col);
    const cb = this.corners(b.row, b.col);
    const shared: number[] = [];
    for (let i = 0; i < 12; i += 2) {
      for (let j = 0; j < 12; j += 2) {
        if (Math.abs(ca[i] - cb[j]) < 0.5 && Math.abs(ca[i + 1] - cb[j + 1]) < 0.5) {
          shared.push(ca[i], ca[i + 1]);
        }
      }
    }
    return shared.length === 4 ? shared : null;
  }
}

export function hexId(row: number, col: number): string {
  return `${String(row).padStart(2, "0")}${String(col).padStart(2, "0")}`;
}

export function parseHexId(id: string): { row: number; col: number } {
  return { row: Number(id.slice(0, 2)), col: Number(id.slice(2)) };
}
