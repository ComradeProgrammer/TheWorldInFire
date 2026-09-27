import { useState, type FormEvent } from "react";
import type {
  AirPoints,
  Airspace,
  BattlePlan,
  CityControlState,
  CombatObjective,
  CombatState,
  GameCommand,
  MovementMode,
  PhaseSnapshot,
  StrikePlan,
  StrikeTargetHex,
  UnitState,
} from "../gameApi";
import { AirStrikePanel } from "./AirStrikePanel";
import { readableId, sideName } from "./unitFormat";
import { UnitCounterIcon } from "./UnitCounterIcon";
import { MOVEMENT_MODES, type MovementPreviewStatus } from "./movementModes";
import { CITY_KIND_NAMES, TERRAIN_NAMES } from "../map/mapData";
import type { HexData, HexsideFeature, MapData } from "../map/mapTypes";

const HEXSIDE_NAMES: Record<HexsideFeature, string> = {
  blocked: "Blocked hexside",
  corpsBoundary: "NATO corps deployment boundary",
  frontBoundary: "WP front deployment boundary",
  allSea: "All-sea hexside",
  causeway: "Causeway",
  majorRiver: "Major river",
  minorRiver: "Minor river",
  danishFerry: "Danish Ferry",
};

function hexsideSummary(map: MapData, id: string): string[] {
  const out: string[] = [];
  for (const side of map.hexsides) {
    if (side.a !== id && side.b !== id) continue;
    const other = side.a === id ? side.b : side.a;
    for (const feature of side.features) out.push(`${HEXSIDE_NAMES[feature]} · ${other}`);
  }
  return out;
}

function UnitRow({ unit, onSelect }: { unit: UnitState; onSelect(): void }) {
  const step = unit.steps[unit.strengthStepIndex];
  return (
    <button type="button" className={`unit-row side-${unit.sideId === "nato" ? "nato" : "pact"}`} onClick={onSelect}>
      <span>{unit.name}</span>
      {unit.disruption && <em className="unit-status" title={readableId(unit.disruption)}>{unit.disruption === "suppressed" ? "S" : "D"}</em>}
      {step && <strong>{step.attack}–{step.defense}–{step.movement}</strong>}
    </button>
  );
}

export interface MovementControl {
  mode: MovementMode;
  status: MovementPreviewStatus;
  onModeChange(mode: MovementMode): void;
}

function PlanningActions({ unit, plan, busy, movement, onCommand }: {
  unit: UnitState;
  plan: BattlePlan;
  busy: boolean;
  movement: MovementControl;
  onCommand(command: GameCommand): void;
}) {
  const resupplySelected = plan.resupplyTargetUnitId === unit.id;
  const movements = plan.movements.filter((entry) => entry.unitId === unit.id);
  const lastMovement = movements[movements.length - 1];
  const orderedEntraining = plan.entrainingUnitIds.includes(unit.id);
  const detrainedThisPlan = plan.detrainedUnitIds.includes(unit.id);

  let railButton;
  if (orderedEntraining) {
    railButton = (
      <button type="button" className="action-toggle active" disabled={busy} onClick={() => onCommand({ type: "detrainUnit", unitId: unit.id })}>
        Undo entrainment order
      </button>
    );
  } else if (detrainedThisPlan && unit.trainStatus === null) {
    railButton = (
      <button type="button" className="action-toggle active" disabled={busy} onClick={() => onCommand({ type: "undoDetrainUnit", unitId: unit.id })}>
        Undo detrain
      </button>
    );
  } else if (unit.trainStatus) {
    railButton = (
      <button type="button" className="action-toggle" disabled={busy} onClick={() => onCommand({ type: "detrainUnit", unitId: unit.id })}>
        Detrain unit
      </button>
    );
  } else {
    railButton = (
      <button type="button" className="action-toggle" disabled={busy} onClick={() => onCommand({ type: "entrainUnit", unitId: unit.id })}>
        Order entrainment
      </button>
    );
  }

  // Availability, reasons, and destinations all come from the Rust core.
  const status = movement.status;
  const modes = status.state === "ready" ? status.modes : [];
  const current = modes.find((entry) => entry.mode === movement.mode);
  let hint: string;
  let warn = false;
  if (status.state === "loading") hint = "Checking legal destinations…";
  else if (status.state === "failed") [hint, warn] = [status.reason, true];
  else if (!current) hint = "";
  else if (current.unavailable) [hint, warn] = [current.unavailable.message, true];
  else if (current.options.length === 0) hint = "No legal destination with this movement mode.";
  else hint = `Right-click a destination on the map (${current.options.length} legal ${current.options.length === 1 ? "hex" : "hexes"}).`;

  return (
    <section className="unit-actions">
      <h3>Planning actions</h3>
      <button
        type="button"
        className={resupplySelected ? "action-toggle active" : "action-toggle"}
        disabled={busy}
        onClick={() => onCommand({ type: "setResupplyTarget", unitId: resupplySelected ? null : unit.id })}
      >
        {resupplySelected ? "Undo resupply selection" : "Select for resupply"}
      </button>

      <div className="movement-command-group">
        <h4>Movement</h4>
        <div className="mode-selector" role="radiogroup" aria-label="Movement mode">
          {MOVEMENT_MODES.map(({ mode, label }) => {
            const entry = modes.find((candidate) => candidate.mode === mode);
            return (
              <button
                key={mode}
                type="button"
                role="radio"
                aria-checked={movement.mode === mode}
                className={movement.mode === mode ? "selected" : undefined}
                title={entry?.unavailable?.message}
                disabled={busy || !entry || entry.unavailable !== null}
                onClick={() => movement.onModeChange(mode)}
              >
                {label}
              </button>
            );
          })}
        </div>
        <p className={warn ? "movement-hint warn" : "movement-hint"}>{hint}</p>
        {lastMovement && (
          <>
            <p className="movement-summary">
              {movements.length > 1 ? `${movements.length} legs · ` : ""}
              {readableId(lastMovement.mode)} to {lastMovement.to}
              {lastMovement.mode !== "airTransport" && (
                <span>
                  {movements.reduce((sum, entry) => sum + entry.cost, 0)} {lastMovement.mode === "rail" ? "rail hexes" : "MP"} used
                </span>
              )}
            </p>
            <button type="button" className="action-toggle active" disabled={busy} onClick={() => onCommand({ type: "undoUnitMovement", unitId: unit.id })}>
              Undo move to {lastMovement.to}
            </button>
          </>
        )}
      </div>

      {railButton}
    </section>
  );
}

function UnitDetails({ unit, plan, activeSideId, planning, busy, movement, onBack, onCommand }: {
  unit: UnitState;
  plan: BattlePlan | null;
  activeSideId?: string;
  planning: boolean;
  busy: boolean;
  movement: MovementControl;
  onBack(): void;
  onCommand(command: GameCommand): void;
}) {
  const step = unit.steps[unit.strengthStepIndex];
  const location = unit.location.type === "hex" ? `Hex ${unit.location.hexId}` : "Strategic Reserve";
  return (
    <div className="unit-detail-view">
      <button type="button" className="panel-back" onClick={onBack}>← Back to hex</button>
      <div className="unit-identity">
        <UnitCounterIcon unit={unit} size={72} />
        <div>
          <span className="detail-eyebrow">{readableId(unit.nationId)}</span>
          <h2>{unit.name}</h2>
          <p>{readableId(unit.unitTypeId)}</p>
        </div>
      </div>
      <dl className="unit-data">
        <div><dt>Location</dt><dd>{location}</dd></div>
        <div><dt>Combat values</dt><dd>{step ? `${step.attack} / ${step.defense} / ${step.movement}` : "—"}</dd></div>
        <div><dt>Formation</dt><dd>{unit.formationId ? readableId(unit.formationId.slice(unit.formationId.lastIndexOf(".") + 1)) : "Independent"}</dd></div>
        <div><dt>Movement supply</dt><dd>{unit.supply.movement ? readableId(unit.supply.movement) : "N/A"}</dd></div>
        <div><dt>Combat supply</dt><dd>{unit.supply.combat ? readableId(unit.supply.combat) : "N/A"}</dd></div>
        <div><dt>Rail status</dt><dd>{unit.trainStatus ? readableId(unit.trainStatus) : "Not entrained"}</dd></div>
        <div><dt>Status</dt><dd className={unit.disruption ? "status-disrupted" : undefined}>{unit.disruption ? readableId(unit.disruption) : "Ready"}</dd></div>
      </dl>
      {unit.traits.length > 0 && <div className="trait-list">{unit.traits.map((trait) => <span key={trait}>{readableId(trait)}</span>)}</div>}
      {planning && plan && unit.sideId === activeSideId ? (
        <PlanningActions unit={unit} plan={plan} busy={busy} movement={movement} onCommand={onCommand} />
      ) : (
        <p className="readonly-note">This unit has no available actions in the current phase.</p>
      )}
    </div>
  );
}

export interface CombatContext {
  state: CombatState;
  objectives: ReadonlyMap<string, CombatObjective>;
  revision: number;
  /** False until the core preview for the current revision has arrived. */
  ready: boolean;
}

export interface StrikeContext {
  plan: StrikePlan;
  points: AirPoints | undefined;
  targets: ReadonlyMap<string, StrikeTargetHex>;
  tacticalHexes: ReadonlySet<string>;
  friendlyHexes: ReadonlySet<string>;
  /** False until the core preview for the current revision has arrived. */
  ready: boolean;
}

/** Combat Phase: the Attack button, enabled only for hexes the core lists as objectives. */
function AttackAction({ hexId, combat, busy, onOpen }: {
  hexId: string;
  combat: CombatContext;
  busy: boolean;
  onOpen(): void;
}) {
  const objective = combat.objectives.get(hexId);
  const report = combat.state.battles.find((battle) => battle.hexId === hexId);
  const pending = combat.state.pendingAdvance;
  let reason: string | null = null;
  if (!combat.ready) reason = "Checking attack options…";
  else if (pending) reason = `Decide the advance into ${pending.hexId} first.`;
  else if (report) reason = "This hex has already been attacked this phase.";
  else if (!objective) reason = "None of your units can attack this hex now.";
  return (
    <div className="attack-action">
      <button type="button" className="attack-button" disabled={busy || reason !== null} title={reason ?? undefined} onClick={onOpen}>
        Attack
      </button>
      {objective?.mandatory && <p className="attack-note must">Marked objective: it must be attacked this phase.</p>}
      {objective?.breakthroughOnly && <p className="attack-note">Breakthrough hex: no defenders, the attackers advance in.</p>}
      {reason && combat.ready && <p className="attack-note">{reason}</p>}
      {report?.result && (
        <p className="attack-note result">
          {report.odds?.finalOdds} · rolled {report.dieRoll} → <strong>{report.result.code}</strong>
        </p>
      )}
    </div>
  );
}

function airspaceOf(strike: StrikeContext, hexId: string): Airspace | null {
  if (!strike.ready) return null;
  if (strike.friendlyHexes.has(hexId)) return "friendly";
  return strike.tacticalHexes.has(hexId) ? "contested" : "enemy";
}

export interface SidePanelProps {
  map: MapData;
  units: UnitState[];
  cities: CityControlState[];
  /** Hexes the core reports as legal attack objectives right now. */
  attackTargets: ReadonlySet<string>;
  currentStep: PhaseSnapshot | null;
  battlePlan: BattlePlan | null;
  selected: HexData | null;
  selectedUnitId: string | null;
  commandBusy: boolean;
  movement: MovementControl;
  /** Offensive Strike Phase context; present only during that phase. */
  strike: StrikeContext | null;
  /** Combat Phase context; present only during that phase. */
  combat: CombatContext | null;
  onGoTo(id: string): boolean;
  onSelectUnit(id: string | null): void;
  onPlanningCommand(command: GameCommand): void;
  /** Opens the Battle Planner for an Objective hex. */
  onOpenBattle(hexId: string): void;
}

export function SidePanel(props: SidePanelProps) {
  const [goTo, setGoTo] = useState("");
  const [goToError, setGoToError] = useState(false);
  const activeSideId = props.currentStep?.actor.type === "side" ? props.currentStep.actor.sideId : undefined;
  const planning = props.currentStep?.phaseId === "battlePlanning";
  const selectedUnit = props.units.find((unit) => unit.id === props.selectedUnitId) ?? null;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const id = goTo.trim().padStart(4, "0");
    const ok = props.map.hexes.some((hex) => hex.id === id) && props.onGoTo(id);
    setGoToError(!ok);
  };

  if (selectedUnit) {
    return (
      <aside className="side-panel">
        <section className="panel control-panel">
          <UnitDetails
            unit={selectedUnit}
            plan={props.battlePlan}
            activeSideId={activeSideId}
            planning={planning}
            busy={props.commandBusy}
            movement={props.movement}
            onBack={() => props.onSelectUnit(null)}
            onCommand={props.onPlanningCommand}
          />
        </section>
      </aside>
    );
  }

  const hex = props.selected;
  const primary = hex ? (hex.city ? CITY_KIND_NAMES[hex.city.kind] : TERRAIN_NAMES[hex.terrain]) : null;
  const occupyingUnits = hex
    ? props.units.filter((unit) => unit.location.type === "hex" && unit.location.hexId === hex.id)
    : [];
  const reserveUnits = planning && activeSideId
    ? props.units.filter((unit) => unit.sideId === activeSideId && unit.location.type === "strategicReserve")
    : [];
  const attackSelected = Boolean(hex && props.battlePlan?.attackTargets.includes(hex.id));
  const canTarget = Boolean(hex && props.attackTargets.has(hex.id));
  const city = hex ? props.cities.find((entry) => entry.hexId === hex.id) : undefined;

  return (
    <aside className="side-panel">
      <section className="panel control-panel">
        <h2>Control Panel</h2>
        <form className="goto primary-goto" onSubmit={submit}>
          <input value={goTo} onChange={(event) => { setGoTo(event.currentTarget.value); setGoToError(false); }} placeholder="Go to hex, e.g. 2417" inputMode="numeric" maxLength={4} className={goToError ? "error" : ""} />
          <button type="submit">Go</button>
        </form>

        {hex ? (
          <div className="hex-context">
            <div className="hex-context-title">
              <div><span className="detail-eyebrow">Selected hex</span><strong>{hex.id}</strong></div>
              <span>{hex.city?.name ?? hex.town ?? ""}</span>
            </div>
            <dl>
              <div><dt>Terrain</dt><dd>{primary}</dd></div>
              {hex.city && <div><dt>Underlying</dt><dd>{TERRAIN_NAMES[hex.terrain]}</dd></div>}
              {hex.commandZone && <div><dt>Command zone</dt><dd>{hex.commandZone}</dd></div>}
              {city && (
                <div>
                  <dt>Control</dt>
                  <dd className={`side-${city.controller === "nato" ? "nato" : "pact"}`}>
                    {sideName(city.controller)} · {city.free ? "Free City" : `Conquered (${sideName(city.owner)} city)`}
                  </dd>
                </div>
              )}
              {hex.city && <div><dt>Organic defense</dt><dd>{city && !city.free ? "None (conquered)" : hex.city.defense}</dd></div>}
              {hex.port && <div><dt>Port capacity</dt><dd>{hex.port}</dd></div>}
              {hex.coastal && <div><dt>Coastal</dt><dd>Yes</dd></div>}
            </dl>
            {hexsideSummary(props.map, hex.id).length > 0 && <div className="hex-feature-list">{hexsideSummary(props.map, hex.id).map((feature) => <span key={feature}>{feature}</span>)}</div>}
            {planning && props.battlePlan && (
              <button
                type="button"
                className={attackSelected ? "hex-objective active" : "hex-objective"}
                disabled={props.commandBusy || (!attackSelected && !canTarget)}
                onClick={() => props.onPlanningCommand({ type: "setAttackTarget", hexId: hex.id, selected: !attackSelected })}
              >
                {attackSelected ? "Undo attack objective" : "Set as attack objective"}
              </button>
            )}
            {props.strike && (
              <AirStrikePanel
                hex={hex}
                units={props.units}
                target={props.strike.targets.get(hex.id)}
                airspace={airspaceOf(props.strike, hex.id)}
                tacticalAllowed={props.strike.tacticalHexes.has(hex.id)}
                plan={props.strike.plan}
                points={props.strike.points}
                busy={props.commandBusy || !props.strike.ready}
                onCommand={props.onPlanningCommand}
              />
            )}
            {props.combat && (
              <AttackAction hexId={hex.id} combat={props.combat} busy={props.commandBusy} onOpen={() => props.onOpenBattle(hex.id)} />
            )}
            <div className="unit-list context-units">
              <h3>Units in hex ({occupyingUnits.length})</h3>
              {occupyingUnits.length > 0
                ? occupyingUnits.map((unit) => <UnitRow key={unit.id} unit={unit} onSelect={() => props.onSelectUnit(unit.id)} />)
                : <p className="empty">No units in this hex.</p>}
            </div>
          </div>
        ) : <p className="empty select-prompt">Select a hex on the map to inspect terrain and units.</p>}

        {reserveUnits.length > 0 && (
          <div className="unit-list reserve-list">
            <h3>Strategic Reserve ({reserveUnits.length})</h3>
            {reserveUnits.map((unit) => <UnitRow key={unit.id} unit={unit} onSelect={() => props.onSelectUnit(unit.id)} />)}
          </div>
        )}
      </section>
    </aside>
  );
}
