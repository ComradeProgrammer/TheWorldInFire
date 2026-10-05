/**
 * Placeholder HUD values used until the Rust game core supplies snapshots.
 * Nothing here is authoritative game state.
 */
export interface HudState {
  turn: number;
  lastTurn: number;
  /** Every step in the turn sequence, including automatic and opposing-side steps. */
  steps: PhaseStep[];
}

export interface PhaseStep {
  id: string;
  label: string;
  state: "done" | "current" | "upcoming";
  side: "nato" | "pact" | "joint";
}

export const PREVIEW_HUD: HudState = {
  turn: 1,
  lastTurn: 14,
  steps: [],
};
