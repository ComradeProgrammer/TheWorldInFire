import type { MovementMode, MovementModeOptions, PlannedMovement } from "../gameApi";

export const MOVEMENT_MODES: { mode: MovementMode; label: string }[] = [
  { mode: "tactical", label: "Tactical" },
  { mode: "march", label: "March" },
  { mode: "rail", label: "Rail" },
  { mode: "airTransport", label: "Air transport" },
  { mode: "paradrop", label: "Paradrop" },
  { mode: "seaTransport", label: "Sea transport" },
];

/**
 * The mode a newly selected unit starts with: the system it already used this
 * phase, otherwise the first system the core reports as usable with destinations.
 * Availability itself always comes from the Rust core.
 */
export function defaultMovementMode(unitId: string, movements: PlannedMovement[], modes: MovementModeOptions[]): MovementMode {
  const moved = movements.filter((movement) => movement.unitId === unitId);
  if (moved.length > 0) return moved[moved.length - 1].mode;
  const usable = modes.filter((entry) => entry.unavailable === null);
  return (usable.find((entry) => entry.options.length > 0) ?? usable[0] ?? modes[0])?.mode ?? "tactical";
}

/** State of the movement preview request for the selected unit. */
export type MovementPreviewStatus = { state: "loading" } | { state: "ready"; modes: MovementModeOptions[] } | { state: "failed"; reason: string };
