import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  activeBattlePlan,
  fetchAirMissionOptions,
  fetchAirPlanningOptions,
  fetchCombatOptions,
  fetchAttackTargetOptions,
  fetchMovementOptions,
  fetchReserveOptions,
  submitGameCommand,
  type GameCommand,
  type AirMissionOptionsResponse,
  type AirPlanningOptionsResponse,
  type CombatOptionsResponse,
  type UnitState,
  type GameSnapshot,
  type NewGameResponse,
  type MovementMode,
  type PhaseSnapshot,
  type RuleRejection,
} from "./gameApi";
import { MapCanvas } from "./map/MapCanvas";
import type { HexData, MapData } from "./map/mapTypes";
import {
  DEFAULT_LAYERS,
  type MapLayerId,
  type MapRenderer,
  type MovementPreview,
  type StrikeOverlay,
  type AirOverlay,
  type CombatOverlay,
} from "./map/render/MapRenderer";
import { BattlePlanner } from "./ui/BattlePlanner";
import { BottomBar } from "./ui/BottomBar";
import { describeCombatEvents, type CombatLogEntry } from "./ui/combatLog";
import { PREVIEW_HUD } from "./ui/hudPreview";
import { defaultMovementMode, type MovementPreviewStatus } from "./ui/movementModes";
import { SettingsDialog } from "./ui/SettingsDialog";
import { SidePanel, type CombatContext } from "./ui/SidePanel";
import { TopBar } from "./ui/TopBar";
import "./App.css";

const PHASE_NAMES: Record<string, string> = {
  jointStatus: "Joint Status",
  jointReinforcement: "Joint Reinforcement",
  preBattle: "Pre-Battle",
  battlePlanning: "Battle Planning",
  jointAirOperations: "Joint Air Operations",
  offensiveStrike: "Offensive Strike",
  combat: "Combat",
  reserve: "Reserve",
  postBattle: "Post-Battle",
};

/** Short labels for the top-bar phase track. */
const PHASE_TRACK_LABELS: Record<string, string> = {
  jointStatus: "Status",
  jointReinforcement: "Reinforce",
  preBattle: "Pre-Battle",
  battlePlanning: "Plan",
  jointAirOperations: "Air Ops",
  offensiveStrike: "Strike",
  combat: "Combat",
  reserve: "Reserve",
  postBattle: "Post-Battle",
};

function phaseName(id: string): string {
  return PHASE_NAMES[id] ?? id;
}

/** Units carrying a Reserve/OMG Marker, for the counter tab. */
function reserveMarkers(snapshot: GameSnapshot): ReadonlySet<string> {
  return new Set(snapshot.battlePlans.flatMap((plan) => plan.reserveUnitIds));
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error && "message" in error) return String(error.message);
  return String(error);
}

/**
 * Adds presentation-only off-map spaces for bases that do not occupy a rules-map
 * hex. They remain ordinary selectable hexes; an in-map base can instead provide
 * `locationHexId` and uses exactly the same installation path.
 */
function mapWithAirBases(
  source: MapData,
  bases: GameSnapshot["airBases"],
  airUnits: GameSnapshot["airUnits"],
): {
  map: MapData;
  displayHexByAirUnitId: ReadonlyMap<string, string>;
} {
  const displayHexByAirUnitId = new Map<string, string>();
  const byId = new Map<string, HexData>(
    source.hexes.map((hex) => [hex.id, { ...hex, installations: [...(hex.installations ?? [])] }]),
  );
  const centers = source.hexes.map((hex) => {
    const columnIndex = source.grid.columnBase - hex.col;
    return source.grid.originX + source.grid.hexWidth * columnIndex + (hex.row % 2 ? source.grid.hexWidth / 2 : 0);
  });
  const middleX = (Math.min(...centers) + Math.max(...centers)) / 2;
  const minColumn = Math.min(...source.hexes.map((hex) => hex.col));
  const maxColumn = Math.max(...source.hexes.map((hex) => hex.col));

  for (const base of bases) {
    const basedUnits = airUnits.filter((unit) => unit.baseId === base.id);
    const inMapHex = base.locationHexId ? byId.get(base.locationHexId) : undefined;
    if (inMapHex) {
      for (const unit of basedUnits) {
        inMapHex.installations = [...(inMapHex.installations ?? []), { kind: "airBase", id: base.id, airUnitId: unit.id }];
        displayHexByAirUnitId.set(unit.id, inMapHex.id);
      }
      continue;
    }

    const anchor = byId.get(base.anchorHexId);
    if (!anchor) continue;
    const anchorX = source.grid.originX
      + source.grid.hexWidth * (source.grid.columnBase - anchor.col)
      + (anchor.row % 2 ? source.grid.hexWidth / 2 : 0);
    // Two columns beyond the global board edge leaves a full empty column
    // between the rules map and this scenario's off-map aircraft spaces.
    const offMapColumn = anchorX < middleX ? maxColumn + 2 : minColumn - 2;
    const firstRow = anchor.row - Math.floor((basedUnits.length - 1) / 2);
    basedUnits.forEach((unit, index) => {
      const row = firstRow + index;
      const displayHexId = `${String(row).padStart(2, "0")}${String(offMapColumn).padStart(2, "0")}`;
      const displayHex: HexData = {
        id: displayHexId,
        row,
        col: offMapColumn,
        terrain: "clear",
        offMap: true,
        installations: [{ kind: "airBase", id: base.id, airUnitId: unit.id }],
      };
      byId.set(displayHexId, displayHex);
      displayHexByAirUnitId.set(unit.id, displayHexId);
    });
  }
  return { map: { ...source, hexes: [...byId.values()] }, displayHexByAirUnitId };
}

/** The game screen for one started game. Remount it (by key) for a new game. */
function App({ game }: { game: NewGameResponse }) {
  const rendererRef = useRef<MapRenderer | null>(null);
  const [selected, setSelected] = useState<HexData | null>(null);
  const [zoom, setZoom] = useState(1);
  const [layers, setLayers] = useState(DEFAULT_LAYERS);
  const [snapshot, setSnapshot] = useState<GameSnapshot | null>(game.snapshot);
  // Rules-map geometry is immutable. Bases without a map location get a normal
  // selectable display hex just outside the nearest edge.
  const airBaseLayout = useMemo(
    () => mapWithAirBases(game.map, game.snapshot.airBases, game.snapshot.airUnits),
    [game],
  );
  const map: MapData = airBaseLayout.map;
  const turnSequence: PhaseSnapshot[] = game.turnSequence;
  const [commandBusy, setCommandBusy] = useState(false);
  const [selectedUnitId, setSelectedUnitId] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [commandNotice, setCommandNotice] = useState<string | null>(null);
  const [logCollapsed, setLogCollapsed] = useState(false);
  const [modeChoice, setModeChoice] = useState<MovementMode | null>(null);
  const [movementStatus, setMovementStatus] = useState<MovementPreviewStatus>({ state: "loading" });
  const [attackTargets, setAttackTargets] = useState<ReadonlySet<string>>(new Set());
  const [reserveOptions, setReserveOptions] = useState<ReadonlyMap<string, RuleRejection | null>>(new Map());
  const [airPlanningOptions, setAirPlanningOptions] = useState<AirPlanningOptionsResponse | null>(null);
  const [airMissionOptions, setAirMissionOptions] = useState<AirMissionOptionsResponse | null>(null);
  const [selectedAirUnitId, setSelectedAirUnitId] = useState<string | null>(null);
  const [selectedAirBaseId, setSelectedAirBaseId] = useState<string | null>(null);
  const [selectedAirCommandUnitId, setSelectedAirCommandUnitId] = useState<string | null>(null);
  const [airQueryBusy, setAirQueryBusy] = useState(false);
  const [combatOptions, setCombatOptions] = useState<CombatOptionsResponse | null>(null);
  const [combatLog, setCombatLog] = useState<CombatLogEntry[]>([]);
  const [battleHexId, setBattleHexId] = useState<string | null>(null);
  // Every unit ever seen, so eliminated units keep their names in logs and plans.
  const [roster, setRoster] = useState<ReadonlyMap<string, UnitState>>(new Map());
  const [airRoster, setAirRoster] = useState<ReadonlyMap<string, GameSnapshot["airUnits"][number]>>(
    new Map(game.snapshot.airUnits.map((unit) => [unit.id, unit])),
  );
  const movementPreviewRef = useRef<MovementPreview | null>(null);
  const strikeOverlayRef = useRef<StrikeOverlay | null>(null);
  const airOverlayRef = useRef<AirOverlay | null>(null);
  const combatOverlayRef = useRef<CombatOverlay | null>(null);

  const hud = useMemo(() => {
    if (!snapshot) return PREVIEW_HUD;
    const steps = turnSequence.flatMap((step, index) => {
      if (step.execution !== "interactive") return [];
      const side = step.actor.type === "all" ? "joint" : step.actor.sideId === "nato" ? "nato" : "pact";
      const state = index < snapshot.turn.stepIndex ? "done" : index === snapshot.turn.stepIndex ? "current" : "upcoming";
      return [{
        id: step.id,
        label: PHASE_TRACK_LABELS[step.phaseId] ?? phaseName(step.phaseId),
        state,
        side,
      } as const];
    });
    return {
      turn: snapshot.turn.gameTurn,
      lastTurn: snapshot.scenario.maxGameTurns,
      steps,
    };
  }, [snapshot, turnSequence]);

  // The selected unit, when it belongs to the side moving now: in Battle
  // Planning, or in the Reserve Phase (where the core allows only marked units).
  const planningUnit = useMemo(() => {
    const step = snapshot?.turn.currentStep;
    if ((step?.phaseId !== "battlePlanning" && step?.phaseId !== "reserve") || step.actor.type !== "side") return null;
    const unit = snapshot?.units.find((candidate) => candidate.id === selectedUnitId);
    return unit && unit.sideId === step.actor.sideId ? unit : null;
  }, [snapshot, selectedUnitId]);

  const onReady = useCallback(
    (renderer: MapRenderer) => {
      rendererRef.current = renderer;
      renderer.setMovementPreview(movementPreviewRef.current);
      if (strikeOverlayRef.current) renderer.setStrikeOverlay(strikeOverlayRef.current);
      if (airOverlayRef.current) renderer.setAirOverlay(airOverlayRef.current);
      if (combatOverlayRef.current) renderer.setCombatOverlay(combatOverlayRef.current);
      if (!snapshot) return;
      renderer.setUnits(snapshot.units, reserveMarkers(snapshot));
      renderer.setBattlePlan(activeBattlePlan(snapshot));
    },
    [snapshot],
  );

  const applyMovementPreview = useCallback((preview: MovementPreview | null) => {
    movementPreviewRef.current = preview;
    rendererRef.current?.setMovementPreview(preview);
  }, []);

  // A newly selected unit starts from its default movement mode.
  useEffect(() => {
    setModeChoice(null);
  }, [planningUnit?.id]);

  // Ask the core which movement systems the unit may use and where each can go.
  // One request per selected unit and accepted command; never per pointer move.
  const revision = snapshot?.revision;
  useEffect(() => {
    if (!planningUnit || revision === undefined) return;
    let active = true;
    setMovementStatus({ state: "loading" });
    fetchMovementOptions(planningUnit.id).then(
      (response) => {
        if (active && response.revision === revision) setMovementStatus({ state: "ready", modes: response.modes });
      },
      (error: unknown) => {
        if (active) setMovementStatus({ state: "failed", reason: errorMessage(error) });
      },
    );
    return () => {
      active = false;
    };
  }, [planningUnit, revision]);

  const movementMode: MovementMode =
    modeChoice ??
    (movementStatus.state === "ready" && planningUnit
      ? defaultMovementMode(planningUnit.id, snapshot?.reserve?.movements ?? (snapshot && activeBattlePlan(snapshot)?.movements) ?? [], movementStatus.modes)
      : "tactical");

  useEffect(() => {
    const entry = movementStatus.state === "ready" ? movementStatus.modes.find((m) => m.mode === movementMode) : undefined;
    if (!planningUnit || !entry) {
      applyMovementPreview(null);
      return;
    }
    applyMovementPreview({
      origin: planningUnit.location.type === "hex" ? planningUnit.location.hexId : null,
      options: new Map(entry.options.map((option) => [option.hexId, option])),
    });
  }, [applyMovementPreview, movementMode, movementStatus, planningUnit]);

  // Legal attack objectives come from the core as well.
  const planningPhase = snapshot?.turn.currentStep?.phaseId === "battlePlanning";
  useEffect(() => {
    setAttackTargets(new Set());
    if (!planningPhase || revision === undefined) return;
    let active = true;
    fetchAttackTargetOptions().then(
      (response) => {
        if (active && response.revision === revision) setAttackTargets(new Set(response.hexIds));
      },
      () => {},
    );
    return () => {
      active = false;
    };
  }, [planningPhase, revision]);

  // Reserve/OMG eligibility for every planning-side unit, from the core.
  useEffect(() => {
    setReserveOptions(new Map());
    if (!planningPhase || revision === undefined) return;
    let active = true;
    fetchReserveOptions().then(
      (response) => {
        if (active && response.revision === revision) {
          setReserveOptions(new Map(response.units.map((entry) => [entry.unitId, entry.unavailable])));
        }
      },
      () => {},
    );
    return () => {
      active = false;
    };
  }, [planningPhase, revision]);

  // Named-aircraft availability and base capacity during Battle Planning.
  useEffect(() => {
    setAirPlanningOptions(null);
    if (!planningPhase || revision === undefined) {
      setSelectedAirUnitId(null);
      return;
    }
    let active = true;
    fetchAirPlanningOptions().then(
      (response) => {
        if (active && response.revision === revision) setAirPlanningOptions(response);
      },
      (error: unknown) => {
        if (active) setCommandNotice(errorMessage(error));
      },
    );
    return () => {
      active = false;
    };
  }, [planningPhase, revision]);

  // Mission targets depend on the selected named aircraft and current revision.
  useEffect(() => {
    setAirMissionOptions(null);
    if (!planningPhase || revision === undefined || !selectedAirUnitId) {
      setAirQueryBusy(false);
      return;
    }
    let active = true;
    setAirQueryBusy(true);
    fetchAirMissionOptions(selectedAirUnitId).then(
      (response) => {
        if (active && response.revision === revision) setAirMissionOptions(response);
      },
      (error: unknown) => {
        if (active) setCommandNotice(errorMessage(error));
      },
    ).finally(() => {
      if (active) setAirQueryBusy(false);
    });
    return () => {
      active = false;
    };
  }, [planningPhase, revision, selectedAirUnitId]);

  const strikePhase = snapshot?.turn.currentStep?.phaseId === "offensiveStrike";

  useEffect(() => {
    if (!snapshot) return;
    setRoster((current) => {
      const next = new Map(current);
      for (const unit of snapshot.units) next.set(unit.id, unit);
      return next;
    });
    setAirRoster((current) => {
      const next = new Map(current);
      for (const unit of snapshot.airUnits) next.set(unit.id, unit);
      return next;
    });
  }, [snapshot]);

  // Combat Phase: attackable hexes and eligible attackers, once per revision.
  const combatPhase = snapshot?.turn.currentStep?.phaseId === "combat";
  useEffect(() => {
    setCombatOptions(null);
    if (!combatPhase || revision === undefined) return;
    let active = true;
    fetchCombatOptions().then(
      (response) => {
        if (active && response.revision === revision) setCombatOptions(response);
      },
      (error: unknown) => {
        if (active) setCommandNotice(errorMessage(error));
      },
    );
    return () => {
      active = false;
    };
  }, [combatPhase, revision]);

  // The Battle Planner stays open while an advance decision is pending.
  useEffect(() => {
    if (!combatPhase) setBattleHexId(null);
  }, [combatPhase]);
  const plannerHexId = combatPhase && snapshot?.combat ? (snapshot.combat.pendingAdvance?.hexId ?? battleHexId) : null;

  const combat = useMemo<CombatContext | null>(() => {
    if (!combatPhase || !snapshot?.combat) return null;
    // Options from an older revision are ignored until the fresh ones arrive.
    const current = combatOptions?.revision === snapshot.revision ? combatOptions : null;
    return {
      state: snapshot.combat,
      objectives: new Map((current?.objectives ?? []).map((objective) => [objective.hexId, objective])),
      revision: snapshot.revision,
      ready: current !== null,
    };
  }, [combatOptions, combatPhase, snapshot]);

  useEffect(() => {
    const overlay: CombatOverlay = {
      objectives: combatOptions?.objectives.map((objective) => ({ hexId: objective.hexId, mandatory: objective.mandatory })) ?? [],
      fought: snapshot?.combat?.attackedHexIds ?? [],
    };
    combatOverlayRef.current = overlay;
    rendererRef.current?.setCombatOverlay(overlay);
  }, [combatOptions, snapshot]);

  useEffect(() => {
    const overlay: StrikeOverlay = {
      targets: [],
      missions: [],
      zones: snapshot?.airInterdictionZones ?? [],
      breakthroughs: snapshot?.breakthroughMarkers.map((marker) => marker.hexId) ?? [],
    };
    strikeOverlayRef.current = overlay;
    rendererRef.current?.setStrikeOverlay(overlay);
  }, [snapshot]);

  useEffect(() => {
    const airUnits = snapshot?.airUnits ?? [];
    const bases = snapshot?.airBases ?? [];
    const sorties: AirOverlay["sorties"] = !(planningPhase || strikePhase) ? [] : (snapshot?.airPlans ?? []).flatMap((plan) =>
      plan.sorties.flatMap((sortie) => {
        const unit = airUnits.find((candidate) => candidate.id === sortie.airUnitId);
        const base = unit ? bases.find((candidate) => candidate.id === unit.baseId) : undefined;
        if (!unit || !base) return [];
        const mission = sortie.mission;
        const centerHexId = mission.type === "airBaseStrike"
          ? (() => {
              const targetUnit = airUnits.find((candidate) => candidate.baseId === mission.airBaseId);
              const targetBase = bases.find((candidate) => candidate.id === mission.airBaseId);
              return (targetUnit && airBaseLayout.displayHexByAirUnitId.get(targetUnit.id)) ?? targetBase?.anchorHexId;
            })()
          : mission.type === "groundStrike"
            ? mission.hexId
            : mission.centerHexId;
        if (!centerHexId) return [];
        const step = unit.steps[unit.strengthStepIndex];
        return [{
          sideId: unit.sideId,
          kind: unit.kind,
          centerHexId,
          baseDisplayHexId: airBaseLayout.displayHexByAirUnitId.get(unit.id) ?? base.anchorHexId,
          radius: unit.kind === "fighter" ? step.combatRadius : unit.kind === "aew" ? step.aewRadius : 0,
          status: sortie.status,
        }];
      }),
    );
    const legalTargetHexIds = airMissionOptions?.kind === "fighterBomber"
      ? airMissionOptions.groundTargets.map((target) => target.hexId)
      : [];
    const overlay: AirOverlay = {
      sorties,
      bases: bases.flatMap((base) => {
        const baseSorties = (snapshot?.airPlans ?? []).flatMap((plan) => plan.sorties)
          .filter((sortie) => airUnits.find((unit) => unit.id === sortie.airUnitId)?.baseId === base.id);
        const groups = new Map<string, typeof airUnits>();
        for (const unit of airUnits.filter((candidate) => candidate.baseId === base.id)) {
          const displayHexId = airBaseLayout.displayHexByAirUnitId.get(unit.id);
          if (!displayHexId) continue;
          groups.set(displayHexId, [...(groups.get(displayHexId) ?? []), unit]);
        }
        return [...groups.entries()].map(([displayHexId, displayedUnits]) => ({
          id: base.id,
          name: base.name,
          sideId: base.sideId,
          anchorHexId: base.anchorHexId,
          displayHexId,
          damage: base.damage,
          closed: base.damage >= 2 || (base.suppressedThroughTurn !== null && (snapshot?.turn.gameTurn ?? 0) <= base.suppressedThroughTurn),
          aircraft: displayedUnits.map((unit) => ({
            id: unit.id,
            kind: unit.kind,
            strengthStepIndex: unit.strengthStepIndex,
            readiness: unit.readiness,
            assigned: baseSorties.some((sortie) => sortie.airUnitId === unit.id),
          })),
        }));
      }),
      legalTargetHexIds,
    };
    airOverlayRef.current = overlay;
    rendererRef.current?.setAirOverlay(overlay);
  }, [airBaseLayout.displayHexByAirUnitId, airMissionOptions, planningPhase, snapshot, strikePhase]);

  useEffect(() => {
    if (snapshot) {
      rendererRef.current?.setUnits(snapshot.units, reserveMarkers(snapshot));
      rendererRef.current?.setBattlePlan(activeBattlePlan(snapshot));
    }
  }, [snapshot]);

  const submitCommand = useCallback(async (command: GameCommand) => {
    if (!snapshot || commandBusy) return;
    setCommandBusy(true);
    try {
      const response = await submitGameCommand(snapshot.revision, command);
      setSnapshot(response.snapshot);
      // Units eliminated by these events are only in the previous roster.
      const roster = [...response.snapshot.units, ...snapshot.units];
      const airRoster = [...response.snapshot.airUnits, ...snapshot.airUnits];
      const airBases = [...response.snapshot.airBases, ...snapshot.airBases];
      setCombatLog((log) => [...log, ...describeCombatEvents(
        response.events,
        roster,
        airRoster,
        airBases,
        (log.length > 0 ? log[log.length - 1].id : 0) + 1,
      )]);
      rendererRef.current?.setUnits(response.snapshot.units, reserveMarkers(response.snapshot));
      rendererRef.current?.setBattlePlan(activeBattlePlan(response.snapshot));
      setCommandNotice(null);
      if (response.events.some((event) => event.type === "airSortiePlanned" || event.type === "airSortieCancelled")) {
        setSelectedAirUnitId(null);
      }
      const arrivals = response.events.flatMap((event) =>
        event.type === "reinforcementsArrived" ? event.units : [],
      );
      if (arrivals.length > 0) rendererRef.current?.focusUnits(arrivals);
      // Keep the moved unit selected and highlight the hex it now occupies.
      const followHex = (hexId: string) => {
        rendererRef.current?.selectById(hexId, false, false);
        setSelected(map?.hexes.find((hex) => hex.id === hexId) ?? null);
      };
      const movement = response.events.find((event) => event.type === "unitMoved");
      if (movement?.type === "unitMoved") followHex(movement.to);
      // Show the Objective hex after a battle so its result and advance choice are visible.
      const fought = response.events.find((event) => event.type === "battleResolved");
      if (fought?.type === "battleResolved") {
        followHex(fought.report.hexId);
        setSelectedUnitId(null);
      }
      const undone = response.events.find((event) => event.type === "unitMovementUndone");
      if (undone?.type === "unitMovementUndone" && undone.restoredLocation.type === "hex") {
        followHex(undone.restoredLocation.hexId);
      }
    } catch (error) {
      setCommandNotice(errorMessage(error));
    } finally {
      setCommandBusy(false);
    }
  }, [commandBusy, map, snapshot]);

  const endPhase = useCallback(() => {
    void submitCommand({ type: "endPhase" });
  }, [submitCommand]);

  // Clicking a hex shows it in the Control Panel; clicking a counter in the selected hex then selects that unit.
  const onSelect = useCallback((hex: HexData | null) => {
    setSelected(hex);
    setSelectedUnitId(null);
    const airBaseInstallation = hex?.installations?.find((installation) => installation.kind === "airBase") ?? null;
    if (airBaseInstallation) {
      setSelectedAirBaseId(airBaseInstallation.id);
      setSelectedAirCommandUnitId(airBaseInstallation.airUnitId ?? null);
      const airUnit = snapshot?.airUnits.find((unit) => unit.id === airBaseInstallation.airUnitId);
      const activeSideId = snapshot?.turn.currentStep?.actor.type === "side" ? snapshot.turn.currentStep.actor.sideId : null;
      setSelectedAirUnitId(planningPhase && airUnit && airUnit.sideId === activeSideId ? airUnit.id : null);
    }
    else if (!selectedAirUnitId) {
      setSelectedAirBaseId(null);
      setSelectedAirCommandUnitId(null);
    }
  }, [planningPhase, selectedAirUnitId, snapshot]);

  const onMoveOrder = useCallback(
    (hexId: string) => {
      if (!planningUnit) return;
      void submitCommand({ type: "moveUnit", unitId: planningUnit.id, destination: hexId, mode: movementMode });
    },
    [movementMode, planningUnit, submitCommand],
  );

  const toggleLayer = (layer: MapLayerId, visible: boolean) => {
    setLayers((prev) => ({ ...prev, [layer]: visible }));
    rendererRef.current?.setLayerVisible(layer, visible);
  };

  const canEndPhase = Boolean(
    snapshot?.status === "inProgress"
    && snapshot.turn.currentStep?.execution === "interactive"
    && !(combatPhase && (!combat?.ready || (combatOptions?.mandatoryRemaining.length ?? 0) > 0)),
  );
  const phaseActionLabel = combatPhase && (combatOptions?.mandatoryRemaining.length ?? 0) > 0
    ? `${combatOptions?.mandatoryRemaining.length} Marked Attack${combatOptions?.mandatoryRemaining.length === 1 ? "" : "s"} Left`
    : "End Phase";

  return (
    <div className={logCollapsed ? "app log-collapsed" : "app"}>
      <TopBar hud={hud} />
      <main className="map-area">
        <MapCanvas
          map={map}
          onReady={onReady}
          onHover={() => {}}
          onSelect={onSelect}
          onUnitSelect={(unitId) => {
            setSelectedUnitId(unitId);
            setSelectedAirUnitId(null);
            setSelectedAirBaseId(null);
            setSelectedAirCommandUnitId(null);
          }}
          onMoveOrder={onMoveOrder}
          onZoom={setZoom}
        />
      </main>
      {snapshot && map && (
        <SidePanel
          map={map}
          units={snapshot.units}
          currentStep={snapshot.turn.currentStep}
          battlePlan={activeBattlePlan(snapshot)}
          selected={selected}
          selectedUnitId={selectedUnitId}
          commandBusy={commandBusy}
          attackTargets={attackTargets}
          cities={snapshot.cities}
          movement={{ mode: movementMode, status: movementStatus, onModeChange: setModeChoice }}
          strike={null}
          air={selectedAirBaseId && selectedAirCommandUnitId ? {
            gameTurn: snapshot.turn.gameTurn,
            phaseId: snapshot.turn.currentStep?.phaseId,
            activeSideId: snapshot.turn.currentStep?.actor.type === "side" ? snapshot.turn.currentStep.actor.sideId : undefined,
            units: snapshot.airUnits,
            knownUnits: [...airRoster.values()],
            bases: snapshot.airBases,
            plans: snapshot.airPlans,
            report: snapshot.airOperationsReport,
            planningOptions: airPlanningOptions?.revision === snapshot.revision ? airPlanningOptions : null,
            missionOptions: airMissionOptions?.revision === snapshot.revision && airMissionOptions.airUnitId === selectedAirUnitId ? airMissionOptions : null,
            selectedAirUnitId,
            selectedHex: selected,
            queryBusy: airQueryBusy,
            commandBusy,
            groundUnits: snapshot.units,
            selectedBaseId: selectedAirBaseId,
            focusedAirUnitId: selectedAirCommandUnitId,
            onSelectAirUnit: setSelectedAirUnitId,
            onCommand: (command) => void submitCommand(command),
            onGoTo: (hexId) => {
              rendererRef.current?.selectById(hexId, true, false);
              setSelected(map.hexes.find((hex) => hex.id === hexId) ?? null);
              setSelectedUnitId(null);
            },
            onClose: () => {
              setSelectedAirUnitId(null);
              setSelectedAirBaseId(null);
              setSelectedAirCommandUnitId(null);
              setSelected(null);
              rendererRef.current?.select(null, false);
            },
          } : null}
          combat={combat}
          onGoTo={(id) => {
            if (!rendererRef.current) return false;
            rendererRef.current.selectById(id);
            return true;
          }}
          onSelectUnit={(unitId) => {
            setSelectedUnitId(unitId);
            if (unitId) {
              setSelectedAirUnitId(null);
              setSelectedAirBaseId(null);
              setSelectedAirCommandUnitId(null);
            }
          }}
          onPlanningCommand={(command) => void submitCommand(command)}
          onOpenBattle={setBattleHexId}
          reserveOptions={reserveOptions}
          reserve={snapshot.reserve}
          canEndPhase={canEndPhase}
          phaseActionLabel={phaseActionLabel}
          phaseActionBusy={commandBusy}
          onEndPhase={endPhase}
          onOpenSettings={() => setSettingsOpen(true)}
        />
      )}
      {snapshot?.combat && map && combat && plannerHexId && (
        <BattlePlanner
          key={plannerHexId}
          map={map}
          hexId={plannerHexId}
          units={snapshot.units}
          roster={roster}
          combat={snapshot.combat}
          objective={combat.objectives.get(plannerHexId)}
          revision={snapshot.revision}
          busy={commandBusy || !combat.ready}
          onCommand={(command) => void submitCommand(command)}
          onClose={() => setBattleHexId(null)}
        />
      )}
      {commandNotice && <div className="command-toast" role="alert"><strong>Command rejected</strong><span>{commandNotice}</span><button type="button" onClick={() => setCommandNotice(null)}>×</button></div>}
      <BottomBar
        plan={snapshot ? activeBattlePlan(snapshot) : null}
        strikePlan={snapshot?.strikePlan ?? null}
        airPlans={snapshot?.airPlans ?? []}
        airUnits={[...airRoster.values()]}
        airBases={snapshot?.airBases ?? []}
        battles={snapshot?.combat?.battles ?? []}
        reserve={snapshot?.reserve ?? null}
        units={[...roster.values()]}
        combatLog={combatLog}
        collapsed={logCollapsed}
        onToggleCollapsed={() => setLogCollapsed((value) => !value)}
      />
      <SettingsDialog
        open={settingsOpen}
        zoom={zoom}
        layers={layers}
        onClose={() => setSettingsOpen(false)}
        onToggleLayer={toggleLayer}
        onZoom={(factor) => rendererRef.current?.zoomBy(factor)}
        onFit={() => rendererRef.current?.fitToView()}
      />
    </div>
  );
}

export default App;
