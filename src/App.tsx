import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  fetchAttackTargetOptions,
  fetchMovementOptions,
  loadInitialGame,
  submitGameCommand,
  type GameCommand,
  type GameSnapshot,
  type MovementMode,
} from "./gameApi";
import { MapCanvas } from "./map/MapCanvas";
import type { HexData, MapData } from "./map/mapTypes";
import { DEFAULT_LAYERS, type MapLayerId, type MapRenderer, type MovementPreview } from "./map/render/MapRenderer";
import { BottomBar } from "./ui/BottomBar";
import { PREVIEW_HUD } from "./ui/hudPreview";
import { defaultMovementMode, type MovementPreviewStatus } from "./ui/movementModes";
import { SettingsDialog } from "./ui/SettingsDialog";
import { SidePanel } from "./ui/SidePanel";
import { TopBar } from "./ui/TopBar";
import "./App.css";

const PHASE_NAMES: Record<string, string> = {
  jointStatus: "Joint Status",
  jointReinforcement: "Joint Reinforcement",
  preBattle: "Pre-Battle",
  battlePlanning: "Battle Planning",
  offensiveStrike: "Offensive Strike",
  combat: "Combat",
  reserve: "Reserve",
  postBattle: "Post-Battle",
};

function phaseName(id: string): string {
  return PHASE_NAMES[id] ?? id;
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error && "message" in error) return String(error.message);
  return String(error);
}

function App() {
  const rendererRef = useRef<MapRenderer | null>(null);
  const [selected, setSelected] = useState<HexData | null>(null);
  const [zoom, setZoom] = useState(1);
  const [layers, setLayers] = useState(DEFAULT_LAYERS);
  const [snapshot, setSnapshot] = useState<GameSnapshot | null>(null);
  const [map, setMap] = useState<MapData | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [commandBusy, setCommandBusy] = useState(false);
  const [selectedUnitId, setSelectedUnitId] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [commandNotice, setCommandNotice] = useState<string | null>(null);
  const [logCollapsed, setLogCollapsed] = useState(false);
  const [modeChoice, setModeChoice] = useState<MovementMode | null>(null);
  const [movementStatus, setMovementStatus] = useState<MovementPreviewStatus>({ state: "loading" });
  const [attackTargets, setAttackTargets] = useState<ReadonlySet<string>>(new Set());
  const movementPreviewRef = useRef<MovementPreview | null>(null);

  useEffect(() => {
    let active = true;
    loadInitialGame().then(
      (next) => {
        if (!active) return;
        setSnapshot(next.snapshot);
        setMap(next.map);
      },
      (error: unknown) => {
        if (!active) return;
        const message = errorMessage(error);
        setLoadError(message);
      },
    );
    return () => {
      active = false;
    };
  }, []);

  const hud = useMemo(() => {
    if (!snapshot) return PREVIEW_HUD;
    const actor = snapshot.turn.currentStep?.actor;
    return {
      ...PREVIEW_HUD,
      scenario: snapshot.scenario.name,
      turn: snapshot.turn.gameTurn,
      lastTurn: snapshot.scenario.maxGameTurns,
      activePlayer:
        actor?.type === "all" ? ("Both" as const) : actor?.sideId === "nato" ? ("NATO" as const) : ("Warsaw Pact" as const),
      phase: snapshot.turn.currentStep ? phaseName(snapshot.turn.currentStep.phaseId) : "Complete",
      resources: [],
    };
  }, [snapshot]);

  // The selected unit, when it belongs to the side currently planning.
  const planningUnit = useMemo(() => {
    const step = snapshot?.turn.currentStep;
    if (step?.phaseId !== "battlePlanning" || step.actor.type !== "side") return null;
    const unit = snapshot?.units.find((candidate) => candidate.id === selectedUnitId);
    return unit && unit.sideId === step.actor.sideId ? unit : null;
  }, [snapshot, selectedUnitId]);

  const onReady = useCallback(
    (renderer: MapRenderer) => {
      rendererRef.current = renderer;
      renderer.setMovementPreview(movementPreviewRef.current);
      if (!snapshot) return;
      renderer.setUnits(snapshot.units);
      renderer.setBattlePlan(snapshot.battlePlan);
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
      ? defaultMovementMode(planningUnit.id, snapshot?.battlePlan ?? null, movementStatus.modes)
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

  useEffect(() => {
    if (snapshot) {
      rendererRef.current?.setUnits(snapshot.units);
      rendererRef.current?.setBattlePlan(snapshot.battlePlan);
    }
  }, [snapshot]);

  const submitCommand = useCallback(async (command: GameCommand) => {
    if (!snapshot || commandBusy) return;
    setCommandBusy(true);
    try {
      const response = await submitGameCommand(snapshot.revision, command);
      setSnapshot(response.snapshot);
      rendererRef.current?.setUnits(response.snapshot.units);
      rendererRef.current?.setBattlePlan(response.snapshot.battlePlan);
      setCommandNotice(null);
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

  // Clicking a hex shows it in the Control Panel; clicking a counter then selects that unit.
  const onSelect = useCallback((hex: HexData | null) => {
    setSelected(hex);
    setSelectedUnitId(null);
  }, []);

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

  return (
    <div className={logCollapsed ? "app log-collapsed" : "app"}>
      <TopBar
        hud={hud}
        canEndPhase={snapshot?.status === "inProgress" && snapshot.turn.currentStep?.execution === "interactive"}
        phaseActionLabel={
          snapshot?.turn.gameTurn === 1 && snapshot.turn.currentStep?.phaseId === "jointStatus"
            ? "Resolve Opening Deployment"
            : "End Phase"
        }
        phaseActionBusy={commandBusy}
        onOpenSettings={() => setSettingsOpen(true)}
        onEndPhase={endPhase}
      />
      <main className="map-area">
        {snapshot && map ? (
          <MapCanvas
            map={map}
            onReady={onReady}
            onHover={() => {}}
            onSelect={onSelect}
            onUnitSelect={setSelectedUnitId}
            onMoveOrder={onMoveOrder}
            onZoom={setZoom}
          />
        ) : (
          <div className={`map-loading${loadError ? " error" : ""}`}>
            {loadError ? `Game creation failed: ${loadError}` : "Creating game and loading map…"}
          </div>
        )}
      </main>
      {snapshot && map && (
        <SidePanel
          map={map}
          units={snapshot.units}
          currentStep={snapshot.turn.currentStep}
          battlePlan={snapshot.battlePlan}
          selected={selected}
          selectedUnitId={selectedUnitId}
          commandBusy={commandBusy}
          attackTargets={attackTargets}
          cities={snapshot.cities}
          movement={{ mode: movementMode, status: movementStatus, onModeChange: setModeChoice }}
          onGoTo={(id) => {
            if (!rendererRef.current) return false;
            rendererRef.current.selectById(id);
            return true;
          }}
          onSelectUnit={setSelectedUnitId}
          onPlanningCommand={(command) => void submitCommand(command)}
        />
      )}
      {commandNotice && <div className="command-toast" role="alert"><strong>Command rejected</strong><span>{commandNotice}</span><button type="button" onClick={() => setCommandNotice(null)}>×</button></div>}
      <BottomBar
        plan={snapshot?.battlePlan ?? null}
        units={snapshot?.units ?? []}
        combatLog={[]}
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
