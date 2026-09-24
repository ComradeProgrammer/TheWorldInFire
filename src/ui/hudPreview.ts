/**
 * Placeholder HUD values used until the Rust game core supplies snapshots.
 * Nothing here is authoritative game state.
 */
export interface HudState {
  scenario: string;
  turn: number;
  lastTurn: number;
  activePlayer: "NATO" | "Warsaw Pact";
  phase: string;
  resources: { label: string; value: string; side: "nato" | "pact" | "neutral" }[];
}

export const PREVIEW_HUD: HudState = {
  scenario: "No scenario loaded",
  turn: 1,
  lastTurn: 14,
  activePlayer: "Warsaw Pact",
  phase: "Joint Status",
  resources: [
    { label: "WP Supply", value: "0", side: "pact" },
    { label: "WP Air", value: "0", side: "pact" },
    { label: "NATO Air", value: "0", side: "nato" },
    { label: "NATO Alert", value: "1", side: "nato" },
    { label: "Victory Pts", value: "0", side: "neutral" },
  ],
};
