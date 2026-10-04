import { invoke } from "@tauri-apps/api/core";
import type { MapData } from "./map/mapTypes";

export interface ScenarioSnapshot {
  id: string;
  name: string;
  maxGameTurns: number;
  mapId: string;
}

/** A registered scenario offered by the core, as listed on the scenario screen. */
export interface ScenarioSummary extends ScenarioSnapshot {
  sides: { id: string; name: string }[];
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
  airPoints: AirPoints[];
  strikePlan: StrikePlan | null;
  airInterdictionZones: AirInterdictionZone[];
  breakthroughMarkers: string[];
  eliminatedUnitIds: string[];
  combat: CombatState | null;
  /** Marked units and their movement during the current Reserve Phase. */
  reserve: ReserveState | null;
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
  disruption: Disruption | null;
}

export type Disruption = "disrupted" | "suppressed";
export type AirPointKind = "tactical" | "operational";
export type AirPointSource = "tactical" | "bonusTactical" | "operational";
export type Airspace = "friendly" | "contested" | "enemy";
export type StrikeResult = "noEffect" | "disrupted" | "stepLoss";

export interface AirPoints {
  sideId: string;
  tactical: number;
  operational: number;
  bonusTactical: number;
}

export interface StrikeResolution {
  dieRoll: number;
  modifier: number;
  modifiedRoll: number;
  result: StrikeResult;
}

export type AirMissionKind = { type: "strike"; unitIds: string[] } | { type: "interdiction" };

export interface AirMission {
  id: number;
  hexId: string;
  kind: AirMissionKind;
  source: AirPointSource;
  resolution: StrikeResolution | null;
}

export interface StrikePlan {
  gameTurn: number;
  sideId: string;
  missions: AirMission[];
  resolved: boolean;
  nextMissionId: number;
}

export interface AirInterdictionZone {
  sideId: string;
  hexId: string;
}

export interface StrikeTargetUnit {
  unitId: string;
  steps: number;
  modifier: number;
  headquarters: boolean;
  alreadyTargeted: boolean;
}

export interface StrikeTargetHex {
  hexId: string;
  airspace: Airspace;
  tacticalAllowed: boolean;
  strikesRemaining: number;
  units: StrikeTargetUnit[];
}

export interface CombatResult {
  code: string;
  attackerSteps: number;
  attackerDisrupted: boolean;
  counterattack: boolean;
  defenderSteps: number;
  defenderDisrupted: boolean;
  retreat: number;
}

export type StrengthModifier =
  | "disrupted"
  | "outOfCombatSupply"
  | "armorIntoCityOrMountain"
  | "minorRiver"
  | "majorRiver"
  | "softUnitCover"
  | "provisionalDefense";

export interface UnitStrength {
  unitId: string;
  printed: number;
  /** Adjusted strength in sixty-fourths. */
  adjusted64ths: number;
  modifiers: StrengthModifier[];
}

export interface ColumnShift {
  reason: "terrain" | "flankAttack" | "concentricAttack" | "surprise" | "offensiveSupport";
  shift: number;
}

export interface BattleOdds {
  attackers: UnitStrength[];
  defenders: UnitStrength[];
  cityDefense: number;
  totalAttack: number;
  totalDefense: number;
  basicColumn: number;
  shifts: ColumnShift[];
  netShift: number;
  finalColumn: number;
  finalOdds: string;
  possibleResults: string[];
}

export interface CounterattackRoll {
  unitId: string;
  targetUnitId: string;
  dieRoll: number;
  disrupted: boolean;
}

export interface BattleReport {
  id: number;
  hexId: string;
  attackingUnitIds: string[];
  odds: BattleOdds | null;
  dieRoll: number | null;
  result: CombatResult | null;
  counterattacks: CounterattackRoll[];
  supportingHqId: string | null;
}

export interface PendingAdvance {
  battleId: number;
  hexId: string;
  eligibleUnitIds: string[];
  conquersFreeCity: boolean;
}

export interface CombatState {
  sideId: string;
  battles: BattleReport[];
  attackedUnitIds: string[];
  attackedHexIds: string[];
  engagedUnitIds: string[];
  supportingHqIds: string[];
  pendingAdvance: PendingAdvance | null;
}

export interface CombatObjective {
  hexId: string;
  eligibleUnitIds: string[];
  mandatory: boolean;
  breakthroughOnly: boolean;
  /** HQs able to give Offensive Support if every eligible unit attacks. */
  supportHqIds: string[];
}

export interface CombatOptionsResponse {
  revision: number;
  objectives: CombatObjective[];
  mandatoryRemaining: string[];
}

export interface BattlePreviewResponse {
  revision: number;
  odds: BattleOdds;
}

/** Combat Results Table columns, weakest to strongest. */
export const ODDS_COLUMNS = ["1:4", "1:3", "1:2", "1:1", "2:1", "3:1", "4:1", "5:1", "6:1", "7:1", "8:1", "9:1", "10:1"];

export interface AirStrikeOptionsResponse {
  revision: number;
  tacticalHexes: string[];
  friendlyHexes: string[];
  targets: StrikeTargetHex[];
}

export type MovementMode = "tactical" | "march" | "rail" | "airTransport" | "paradrop" | "seaTransport";

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

export interface ReserveOptionsResponse {
  revision: number;
  /** Every planning-side unit on the map, with the reason it cannot be marked, if any. */
  units: { unitId: string; unavailable: RuleRejection | null }[];
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
  resupplyTargetUnitIds: string[];
  attackTargets: string[];
  movements: PlannedMovement[];
  entrainingUnitIds: string[];
  detrainedUnitIds: string[];
  airliftStepsUsed: number;
  /** Units under a Reserve (NATO) or OMG (WP) Marker. */
  reserveUnitIds: string[];
}

/** The acting side's Reserve Phase: marked units and their movement. */
export interface ReserveState {
  sideId: string;
  unitIds: string[];
  movements: PlannedMovement[];
}

export type GameCommand =
  | { type: "endPhase" }
  | { type: "setResupplyTarget"; unitId: string; selected: boolean }
  | { type: "setAttackTarget"; hexId: string; selected: boolean }
  | { type: "moveUnit"; unitId: string; destination: string; mode: MovementMode }
  | { type: "setReserve"; unitId: string; selected: boolean }
  | { type: "undoUnitMovement"; unitId: string }
  | { type: "entrainUnit"; unitId: string }
  | { type: "detrainUnit"; unitId: string }
  | { type: "undoDetrainUnit"; unitId: string }
  | { type: "planAirStrike"; hexId: string; unitIds: string[]; airPoint: AirPointKind }
  | { type: "planAirInterdiction"; hexId: string; airPoint: AirPointKind }
  | { type: "cancelAirMission"; missionId: number }
  | { type: "resolveAirStrikes" }
  | { type: "resolveBattle"; hexId: string; unitIds: string[]; supportingHqId: string | null }
  | { type: "advanceAfterCombat"; unitIds: string[] };

export type GameEvent =
  | { type: "phaseEnded"; gameTurn: number; step: PhaseSnapshot }
  | { type: "phaseStarted"; gameTurn: number; step: PhaseSnapshot }
  | { type: "gameTurnStarted"; gameTurn: number }
  | { type: "reinforcementsArrived"; gameTurn: number; units: UnitState[] }
  | { type: "preBattleSupplyChecked"; gameTurn: number; sideId: string; units: unknown[] }
  | { type: "resupplyTargetSet"; sideId: string; unitId: string; selected: boolean }
  | { type: "attackTargetSet"; sideId: string; hexId: string; selected: boolean }
  | { type: "unitMoved"; unitId: string; from: UnitLocation; to: string; mode: MovementMode; cost: number; path: string[] }
  | { type: "unitMovementUndone"; movement: PlannedMovement; restoredLocation: UnitLocation }
  | { type: "cityControlChanged"; hexId: string; controller: string; free: boolean }
  | { type: "trainStatusChanged"; unitId: string; status: UnitState["trainStatus"] }
  | { type: "unitsResupplied"; hexId: string; unitIds: string[] }
  | { type: "airPointsReset"; airPoints: AirPoints[] }
  | { type: "airMissionPlanned"; sideId: string; mission: AirMission }
  | { type: "airMissionCancelled"; sideId: string; missionId: number }
  | { type: "airStrikeResolved"; sideId: string; missionId: number; hexId: string; unitIds: string[]; resolution: StrikeResolution }
  | { type: "airInterdictionZonePlaced"; sideId: string; hexId: string }
  | { type: "airInterdictionZonesRemoved"; sideId: string; hexIds: string[] }
  | { type: "unitSupplyChanged"; unitId: string; supply: UnitState["supply"] }
  | { type: "reserveStatusChanged"; sideId: string; unitId: string; selected: boolean }
  | { type: "reserveMarkersRemoved"; sideId: string; unitIds: string[] }
  | { type: "unitDisruptionChanged"; unitId: string; disruption: Disruption | null }
  | { type: "unitStepLost"; unitId: string; strengthStepIndex: number }
  | { type: "unitEliminated"; unitId: string; hexId: string }
  | { type: "breakthroughMarkerPlaced"; hexId: string }
  | { type: "breakthroughMarkersRemoved"; hexIds: string[] }
  | { type: "unitWithdrawn"; gameTurn: number; unitId: string }
  | { type: "battleResolved"; sideId: string; report: BattleReport }
  | { type: "unitRetreated"; unitId: string; from: string; path: string[] }
  | { type: "advanceOffered"; pending: PendingAdvance }
  | { type: "unitsAdvanced"; sideId: string; hexId: string; unitIds: string[] }
  | { type: "gameCompleted"; gameTurn: number };

export interface CommandResponse {
  revision: number;
  events: GameEvent[];
  snapshot: GameSnapshot;
}

export interface NewGameResponse {
  snapshot: GameSnapshot;
  map: MapData;
  /** The scenario's steps for one game turn, in order; sent once with the map. */
  turnSequence: PhaseSnapshot[];
}

/** Every scenario registered in the core. */
export function listScenarios(): Promise<ScenarioSummary[]> {
  return invoke<ScenarioSummary[]>("list_scenarios");
}

/** Creates a game of the chosen scenario, returning its snapshot and authoritative map. */
export function startNewGame(scenarioId: string): Promise<NewGameResponse> {
  return invoke<NewGameResponse>("new_game", { scenarioId });
}

/** Closes the desktop application. */
export function quitApp(): Promise<void> {
  return invoke<void>("quit_app");
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

/** Read-only preview of which planning-side units may take a Reserve/OMG Marker. */
export function fetchReserveOptions(): Promise<ReserveOptionsResponse> {
  return invoke<ReserveOptionsResponse>("reserve_options");
}

/** Read-only preview of the phasing side's Air Strike Segment choices. */
export function fetchAirStrikeOptions(): Promise<AirStrikeOptionsResponse> {
  return invoke<AirStrikeOptionsResponse>("air_strike_options");
}

/** Read-only preview of the Combat Phase: attackable hexes and eligible attackers. */
export function fetchCombatOptions(): Promise<CombatOptionsResponse> {
  return invoke<CombatOptionsResponse>("combat_options");
}

/** Read-only odds for a proposed attack. */
export function fetchBattlePreview(hexId: string, unitIds: string[], supportingHqId: string | null): Promise<BattlePreviewResponse> {
  return invoke<BattlePreviewResponse>("battle_preview", { request: { hexId, unitIds, supportingHqId } });
}
