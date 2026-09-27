/**
 * Placeholder HUD values used until the Rust game core supplies snapshots.
 * Nothing here is authoritative game state.
 */
export interface HudState {
  scenario: string;
  turn: number;
  lastTurn: number;
  activePlayer: "NATO" | "Warsaw Pact" | "Both";
  phase: string;
  /** The acting side's visible steps this game turn, as a progress track. */
  steps: PhaseStep[];
  resources: { label: string; value: string; side: "nato" | "pact" | "neutral" }[];
}

export interface PhaseStep {
  id: string;
  label: string;
  state: "done" | "current" | "upcoming";
}

export const PREVIEW_HUD: HudState = {
  scenario: "No scenario loaded",
  turn: 1,
  lastTurn: 14,
  activePlayer: "Warsaw Pact",
  phase: "Joint Status",
  steps: [],
  resources: [
    { label: "WP Supply", value: "0", side: "pact" },
    { label: "WP Air", value: "0", side: "pact" },
    { label: "NATO Air", value: "0", side: "nato" },
    { label: "NATO Alert", value: "1", side: "nato" },
    { label: "Victory Pts", value: "0", side: "neutral" },
  ],
};
