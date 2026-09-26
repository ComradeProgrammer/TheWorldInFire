import { invoke } from "@tauri-apps/api/core";
import type { MapData } from "./map/mapTypes";

export interface ScenarioSnapshot {
  id: string;
  name: string;
  maxGameTurns: number;
  mapId: string;
}

export interface PhaseActorSnapshot {
  type: "all" | "side";
  sideId?: string;
}

export interface PhaseSnapshot {
  id: string;
  phaseId: string;
  actor: PhaseActorSnapshot;
  execution: "interactive" | "conditional" | "automatic";
}

export interface GameSnapshot {
  protocolVersion: number;
  gameId: string;
  revision: number;
  scenario: ScenarioSnapshot;
  status: "inProgress" | "completed";
  turn: {
    gameTurn: number;
    stepIndex: number;
    currentStep: PhaseSnapshot | null;
  };
  units: UnitState[];
  battlePlan: BattlePlan | null;
  cities: CityControlState[];
  pendingDecision: { kind: string } | null;
}

export interface UnitStep {
  attack: number;
  defense: number;
  movement: number;
}

export type UnitLocation = { type: "hex"; hexId: string } | { type: "strategicReserve" };

export interface UnitState {
  id: string;
  name: string;
  sideId: "warsawPact" | "nato" | string;
  nationId: string;
  unitTypeId: string;
  formationId: string | null;
  traits: string[];
  steps: UnitStep[];
  strengthStepIndex: number;
  location: UnitLocation;
  supply: {
    headquarters: "supplied" | "outOfSupply" | null;
    movement: "supplied" | "outOfSupply" | null;
    combat: "supplied" | "outOfSupply" | null;
  };
  trainStatus: "entraining" | "entrained" | null;
}

export type MovementMode = "tactical" | "march" | "rail" | "airTransport";

export interface PlannedMovement {
  unitId: string;
  from: UnitLocation;
  to: string;
  mode: MovementMode;
  cost: number;
  path: string[];
  cityControlChanges: CityControlChange[];
}

export interface MovementOption {
  hexId: string;
  cost: number;
  path: string[];
}

export interface RuleRejection {
  code: string;
  message: string;
}

/** Core-computed availability and legal destinations for one movement system. */
export interface MovementModeOptions {
  mode: MovementMode;
  unavailable: RuleRejection | null;
  options: MovementOption[];
}

export interface MovementOptionsResponse {
  revision: number;
  unitId: string;
  modes: MovementModeOptions[];
}

export interface AttackTargetOptionsResponse {
  revision: number;
  hexIds: string[];
}

export interface CityControlState {
  hexId: string;
  owner: string;
  controller: string;
  free: boolean;
}

export interface CityControlChange {
  hexId: string;
  previousController: string;
}

export interface BattlePlan {
  gameTurn: number;
  sideId: string;
  resupplyTargetUnitId: string | null;
  attackTargets: string[];
  movements: PlannedMovement[];
  entrainingUnitIds: string[];
  detrainedUnitIds: string[];
  airliftStepsUsed: number;
}

export type GameCommand =
  | { type: "endPhase" }
  | { type: "setResupplyTarget"; unitId: string | null }
  | { type: "setAttackTarget"; hexId: string; selected: boolean }
  | { type: "moveUnit"; unitId: string; destination: string; mode: MovementMode }
  | { type: "undoUnitMovement"; unitId: string }
  | { type: "entrainUnit"; unitId: string }
  | { type: "detrainUnit"; unitId: string }
  | { type: "undoDetrainUnit"; unitId: string };

export type GameEvent =
  | { type: "phaseEnded"; gameTurn: number; step: PhaseSnapshot }
  | { type: "phaseStarted"; gameTurn: number; step: PhaseSnapshot }
  | { type: "gameTurnStarted"; gameTurn: number }
  | { type: "reinforcementsArrived"; gameTurn: number; units: UnitState[] }
  | { type: "preBattleSupplyChecked"; gameTurn: number; sideId: string; units: unknown[] }
  | { type: "resupplyTargetSet"; sideId: string; unitId: string | null }
  | { type: "attackTargetSet"; sideId: string; hexId: string; selected: boolean }
  | { type: "unitMoved"; unitId: string; from: UnitLocation; to: string; mode: MovementMode; cost: number; path: string[] }
  | { type: "unitMovementUndone"; movement: PlannedMovement; restoredLocation: UnitLocation }
  | { type: "cityControlChanged"; hexId: string; controller: string; free: boolean }
  | { type: "trainStatusChanged"; unitId: string; status: UnitState["trainStatus"] }
  | { type: "unitsResupplied"; hexId: string; unitIds: string[] }
  | { type: "gameCompleted"; gameTurn: number };

export interface CommandResponse {
  revision: number;
  events: GameEvent[];
  snapshot: GameSnapshot;
}

export interface NewGameResponse {
  snapshot: GameSnapshot;
  map: MapData;
}

let initialGame: Promise<NewGameResponse> | null = null;

/** Creates the initial desktop game once, including its authoritative map. */
export function loadInitialGame(): Promise<NewGameResponse> {
  initialGame ??= invoke<NewGameResponse>("new_game", { scenarioId: "nato-baltap-1983" });
  return initialGame;
}

/** Submits an intent against the current authoritative revision. */
export function submitGameCommand(expectedRevision: number, command: GameCommand): Promise<CommandResponse> {
  return invoke<CommandResponse>("submit_game_command", {
    request: { expectedRevision, command },
  });
}

/** Read-only preview: availability and legal destinations for every movement system of one unit. */
export function fetchMovementOptions(unitId: string): Promise<MovementOptionsResponse> {
  return invoke<MovementOptionsResponse>("movement_options", { request: { unitId } });
}

/** Read-only preview of the hexes the planning side may mark as attack objectives. */
export function fetchAttackTargetOptions(): Promise<AttackTargetOptionsResponse> {
  return invoke<AttackTargetOptionsResponse>("attack_target_options");
}
