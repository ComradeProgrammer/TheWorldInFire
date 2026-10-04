import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  fetchBattlePreview,
  ODDS_COLUMNS,
  type BattleOdds,
  type BattleReport,
  type ColumnShift,
  type CombatObjective,
  type CombatState,
  type GameCommand,
  type KnownShiftReason,
  type KnownStrengthModifier,
  type StrengthModifier,
  type UnitState,
  type UnitStrength,
} from "../gameApi";
import { HexGrid, hexId as toHexId, neighborCoords } from "../map/hexGrid";
import { CITY_KIND_NAMES, TERRAIN_NAMES } from "../map/mapData";
import type { HexData, MapData } from "../map/mapTypes";
import { COLORS } from "../map/render/style";
import { sideName } from "./unitFormat";
import { UnitCounterIcon } from "./UnitCounterIcon";

const MODIFIER_LABELS: Record<KnownStrengthModifier, string> = {
  disrupted: "Disrupted ½",
  outOfCombatSupply: "Out of supply ½",
  armorIntoCityOrMountain: "Armor into city/mountain ½",
  minorRiver: "Minor river ¾",
  majorRiver: "Major river ½",
  softUnitCover: "Soft unit in cover ×2",
  provisionalDefense: "HQ provisional",
};

const SHIFT_LABELS: Record<KnownShiftReason, string> = {
  terrain: "Terrain",
  flankAttack: "Flank attack",
  concentricAttack: "Concentric attack",
  surprise: "Surprise",
  offensiveSupport: "Offensive Support",
};

/** Label for a printed modifier, or the raw name of one added by another rules plugin. */
function modifierLabel(modifier: StrengthModifier): string {
  return (MODIFIER_LABELS as Record<string, string>)[modifier] ?? modifier;
}

/** Label for a printed shift reason, or the raw name of one added by another rules plugin. */
function shiftLabel(reason: ColumnShift["reason"]): string {
  return (SHIFT_LABELS as Record<string, string>)[reason] ?? reason;
}

const SIDE_COLORS = { nato: "#6fa3e0", pact: "#e06a5c" } as const;

function tone(sideId: string): "nato" | "pact" {
  return sideId === "nato" ? "nato" : "pact";
}

function signed(value: number): string {
  return value >= 0 ? `+${value}` : `${value}`;
}

function strength(sixtyFourths: number): string {
  const value = sixtyFourths / 64;
  return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(/0$/, "");
}

function cssColor(value: number): string {
  return `#${value.toString(16).padStart(6, "0")}`;
}

function hexOf(unit: UnitState | undefined): string | null {
  return unit?.location.type === "hex" ? unit.location.hexId : null;
}

// ------------------------------------------------------------------ mini-map

/** Counter size (in hex half-widths) and columns for `count` units side by side in one hex. */
function counterGrid(count: number): { size: number; columns: number } {
  if (count <= 1) return { size: 0.95, columns: 1 };
  if (count <= 4) return { size: 0.74, columns: 2 };
  return { size: 0.56, columns: 3 };
}

function boundsOf(points: number[]) {
  let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity];
  for (let i = 0; i < points.length; i += 2) {
    minX = Math.min(minX, points[i]);
    maxX = Math.max(maxX, points[i]);
    minY = Math.min(minY, points[i + 1]);
    maxY = Math.max(maxY, points[i + 1]);
  }
  return { minX, minY, maxX, maxY };
}

const polyline = (points: number[]) => `M${points.slice(0, 2).join(",")}L${points.slice(2).join(",")}`;

/**
 * The Objective hex and its six neighbours, drawn like the main map (terrain,
 * water, city outlines, rivers) with every unit in them as its counter.
 */
function BattleMiniMap({ map, centerId, units, attackerSideId, activeIds }: {
  map: MapData;
  centerId: string;
  units: UnitState[];
  attackerSideId: string;
  /** Attackers currently committed; other attackers draw dimmed. */
  activeIds: ReadonlySet<string>;
}) {
  const grid = useMemo(() => new HexGrid(map.grid), [map.grid]);
  const cluster = useMemo(() => {
    const byId = new Map(map.hexes.map((hex) => [hex.id, hex]));
    const center = byId.get(centerId);
    if (!center) return [];
    const around = neighborCoords(center.row, center.col).map(({ row, col }) => byId.get(toHexId(row, col)));
    return [center, ...around].filter((hex): hex is HexData => Boolean(hex));
  }, [centerId, map.hexes]);

  // Map features that reach into the cluster, clipped to it when drawn.
  const features = useMemo(() => {
    const box = boundsOf(cluster.flatMap((hex) => grid.corners(hex.row, hex.col)));
    const touches = (points: number[]) => {
      const b = boundsOf(points);
      return b.maxX >= box.minX && b.minX <= box.maxX && b.maxY >= box.minY && b.minY <= box.maxY;
    };
    return {
      box,
      water: map.water.filter((water) => touches(water.outer)),
      cities: map.cities.filter((city) => touches(city.points)),
      rivers: map.lines.filter((line) => (line.kind === "majorRiver" || line.kind === "minorRiver") && touches(line.points)),
      coast: map.lines.filter((line) => line.kind === "coast" && touches(line.points)),
    };
  }, [cluster, grid, map.cities, map.lines, map.water]);

  if (cluster.length === 0) return null;
  const hw = grid.halfWidth;
  const ry = grid.radiusY;
  const pad = hw * 0.1;
  const { box } = features;
  const viewBox = `${box.minX - pad} ${box.minY - pad} ${box.maxX - box.minX + 2 * pad} ${box.maxY - box.minY + 2 * pad}`;
  const points = (values: number[]) => values.join(" ");
  const clipId = `battle-minimap-clip-${centerId}`;
  const center = cluster[0];

  return (
    <svg className="battle-minimap" viewBox={viewBox} role="img" aria-label={`Hex ${centerId} and its neighbours`}>
      <defs>
        <clipPath id={clipId}>
          {cluster.map((hex) => <polygon key={hex.id} points={points(grid.corners(hex.row, hex.col))} />)}
        </clipPath>
      </defs>
      <g clipPath={`url(#${clipId})`}>
        {cluster.map((hex) => (
          <polygon key={hex.id} points={points(grid.corners(hex.row, hex.col))} fill={cssColor(COLORS[hex.terrain])} />
        ))}
        {features.water.map((water, index) => (
          <path
            key={index}
            d={[water.outer, ...(water.holes ?? [])].map((ring) => `${polyline(ring)}Z`).join(" ")}
            fill={cssColor(COLORS.sea)}
            fillRule="evenodd"
          />
        ))}
        {features.coast.map((line, index) => (
          <path key={index} d={polyline(line.points)} fill="none" stroke={cssColor(COLORS.coast)} strokeWidth={2.5} strokeOpacity={0.9} />
        ))}
        {features.cities.map((city, index) => (
          <polygon
            key={index}
            points={points(city.points)}
            fill={cssColor(COLORS[`${city.kind}City`])}
            stroke={cssColor(COLORS[`${city.kind}CityEdge`])}
            strokeWidth={1.8}
          />
        ))}
        {features.rivers.map((line, index) => {
          const major = line.kind === "majorRiver";
          return (
            <path
              key={index}
              d={polyline(line.points)}
              fill="none"
              stroke={cssColor(major ? COLORS.majorRiver : COLORS.minorRiver)}
              strokeWidth={major ? 8 : 3.5}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          );
        })}
      </g>
      {cluster.map((hex) => {
        const { x, y } = grid.center(hex.row, hex.col);
        return (
          <g key={hex.id}>
            <polygon points={points(grid.corners(hex.row, hex.col))} fill="none" stroke={cssColor(COLORS.grid)} strokeOpacity={0.55} strokeWidth={2} />
            <text x={x} y={y - ry * 0.7} className="minimap-hex-id" fontSize={hw * 0.2}>{hex.id}</text>
            {(hex.city?.name ?? hex.town) && (
              <text x={x} y={y + ry * 0.78} className="minimap-place" fontSize={hw * 0.17}>{hex.city?.name ?? hex.town}</text>
            )}
          </g>
        );
      })}
      <polygon
        points={points(grid.corners(center.row, center.col, 0.95))}
        fill="none"
        stroke="#ff3b30"
        strokeWidth={hw * 0.07}
        strokeLinejoin="round"
      />
      {cluster.map((hex) => {
        const { x, y } = grid.center(hex.row, hex.col);
        const here = units.filter((unit) => hexOf(unit) === hex.id);
        const { size: relative, columns } = counterGrid(here.length);
        const size = hw * relative;
        const gap = hw * 0.05;
        const rows = Math.ceil(here.length / columns);
        const top = y - (rows * size + (rows - 1) * gap) / 2 + ry * 0.03;
        return here.map((unit, index) => {
          const row = Math.floor(index / columns);
          const inRow = Math.min(columns, here.length - row * columns);
          const left = x - (inRow * size + (inRow - 1) * gap) / 2 + (index % columns) * (size + gap);
          const dim = unit.sideId === attackerSideId && !activeIds.has(unit.id);
          return (
            <g key={unit.id} data-chip={unit.id} opacity={dim ? 0.45 : 1}>
              <UnitCounterIcon unit={unit} size={size} x={left} y={top + row * (size + gap)} />
            </g>
          );
        });
      })}
    </svg>
  );
}

// ------------------------------------------------------------------ unit rows

interface ForceRowProps {
  unitId: string;
  name: string;
  sideId: string;
  /** Which edge the link to the mini-map leaves from. */
  anchor: "left" | "right";
  active: boolean;
  detail: string;
  value: string;
  checkbox?: { checked: boolean; disabled: boolean; onChange(): void };
}

function ForceRow({ unitId, name, sideId, anchor, active, detail, value, checkbox }: ForceRowProps) {
  const body = (
    <>
      {checkbox && <input type="checkbox" checked={checkbox.checked} disabled={checkbox.disabled} onChange={checkbox.onChange} />}
      <span className="force-name">
        {name}
        {detail && <small>{detail}</small>}
      </span>
      <strong>{value}</strong>
    </>
  );
  const props = {
    className: `force-row side-${tone(sideId)}${active ? "" : " idle"}`,
    "data-link": unitId,
    "data-anchor": anchor,
    "data-side": tone(sideId),
    "data-active": active ? "1" : "0",
  };
  return checkbox ? <label {...props}>{body}</label> : <div {...props}>{body}</div>;
}

function strengthDetail(entry: UnitStrength | undefined, location: string | null): string {
  const parts = entry?.modifiers.map(modifierLabel) ?? [];
  if (location) parts.unshift(location);
  return parts.join(" · ");
}

// ------------------------------------------------------------------ odds and results

function OddsSummary({ odds }: { odds: BattleOdds }) {
  return (
    <div className="battle-odds">
      <div className="odds-headline">
        <span><b>{odds.totalAttack}</b> vs <b>{odds.totalDefense}</b></span>
        <span>{ODDS_COLUMNS[odds.basicColumn]}{odds.netShift !== 0 && ` ${signed(odds.netShift)}`}</span>
        <strong>{odds.finalOdds}</strong>
      </div>
      {odds.shifts.length > 0 && (
        <p className="odds-shifts">
          {odds.shifts.map((shift) => `${shiftLabel(shift.reason)} ${signed(shift.shift)}`).join(" · ")}
          {odds.shifts.reduce((sum, shift) => sum + shift.shift, 0) !== odds.netShift && " (capped)"}
        </p>
      )}
      <ol className="crt-column" aria-label="Possible results by die roll">
        {odds.possibleResults.map((result, index) => (
          <li key={index}><span>{index + 1}</span>{result}</li>
        ))}
      </ol>
    </div>
  );
}

function ResultSummary({ report, nameOf }: { report: BattleReport; nameOf(id: string): string }) {
  if (!report.result || report.dieRoll === null) {
    return <p className="battle-report">No defenders: the attackers may advance into the hex.</p>;
  }
  return (
    <div className="battle-report">
      <p className="battle-roll">
        {report.odds?.finalOdds} · rolled <b>{report.dieRoll}</b> → <strong>{report.result.code}</strong>
      </p>
      {report.supportingHqId && <p className="support-note">Offensive Support: {nameOf(report.supportingHqId)}</p>}
      {report.counterattacks.map((roll, index) => (
        <p key={index} className="counterattack">
          Counterattack on {nameOf(roll.targetUnitId)}: {roll.dieRoll} → {roll.disrupted ? "Disrupted" : "no effect"}
        </p>
      ))}
    </div>
  );
}

// ------------------------------------------------------------------ links

interface Link {
  key: string;
  points: string;
  end: { x: number; y: number };
  side: "nato" | "pact";
  active: boolean;
}

/** Measures each unit row and its mini-map chip, and joins them with an elbowed polyline. */
function measureLinks(body: HTMLElement): Link[] {
  const origin = body.getBoundingClientRect();
  const chips = new Map<string, Element>();
  body.querySelectorAll("[data-chip]").forEach((chip) => chips.set(chip.getAttribute("data-chip")!, chip));
  const links: Link[] = [];
  body.querySelectorAll<HTMLElement>("[data-link]").forEach((row) => {
    const chip = chips.get(row.dataset.link!);
    if (!chip) return;
    const r = row.getBoundingClientRect();
    const list = row.closest(".force-list")?.getBoundingClientRect();
    const y = r.top + r.height / 2;
    // Rows scrolled out of their list have nothing to point from.
    if (list && (y < list.top || y > list.bottom)) return;
    const c = chip.getBoundingClientRect();
    const right = row.dataset.anchor === "right";
    const x = (right ? r.right : r.left) - origin.left;
    const elbow = right ? x + 14 : x - 14;
    // Stop at the counter's nearest edge so the line never covers its face.
    const rowY = y - origin.top;
    const end = {
      x: Math.min(Math.max(elbow, c.left - origin.left), c.right - origin.left),
      y: Math.min(Math.max(rowY, c.top - origin.top), c.bottom - origin.top),
    };
    links.push({
      key: `${row.dataset.anchor}:${row.dataset.link}`,
      points: `${x},${rowY} ${elbow},${rowY} ${end.x},${end.y}`,
      end,
      side: row.dataset.side === "nato" ? "nato" : "pact",
      active: row.dataset.active === "1",
    });
  });
  return links;
}

// ------------------------------------------------------------------ dialog

export interface BattlePlannerProps {
  map: MapData;
  hexId: string;
  /** Units currently on the map. */
  units: UnitState[];
  /** Every unit ever seen, so eliminated units keep their names. */
  roster: ReadonlyMap<string, UnitState>;
  combat: CombatState;
  /** Core-provided objective for this hex while it may still be attacked. */
  objective: CombatObjective | undefined;
  revision: number;
  busy: boolean;
  onCommand(command: GameCommand): void;
  onClose(): void;
}

/**
 * Combat Phase dialog for one Objective hex: defenders on the left, attackers
 * on the right, and the hex with its neighbours in between. The core computes
 * the odds and validates the order; this only collects the attacker's choices.
 */
export function BattlePlanner(props: BattlePlannerProps) {
  const { map, hexId, units, roster, combat, objective, revision, busy, onCommand, onClose } = props;
  const hex = map.hexes.find((candidate) => candidate.id === hexId);
  const report = combat.battles.find((battle) => battle.hexId === hexId);
  const pending = combat.pendingAdvance?.hexId === hexId ? combat.pendingAdvance : null;
  const nameOf = useCallback((id: string) => roster.get(id)?.name ?? id, [roster]);
  const unitById = useMemo(() => new Map(units.map((unit) => [unit.id, unit])), [units]);

  const [chosen, setChosen] = useState<string[]>(objective?.eligibleUnitIds ?? []);
  // Offensive Support is a once-per-phase choice, so it starts unselected.
  const [supportHq, setSupportHq] = useState<string | null>(null);
  const [advancing, setAdvancing] = useState<string[]>([]);
  const pendingKey = pending?.eligibleUnitIds.join(",") ?? "";
  useEffect(() => setAdvancing(pendingKey ? pendingKey.split(",") : []), [pendingKey]);
  const toggle = (list: string[], id: string) => (list.includes(id) ? list.filter((entry) => entry !== id) : [...list, id]);

  // Odds come from the core for exactly the ticked units.
  const [odds, setOdds] = useState<BattleOdds | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  useEffect(() => {
    setOdds(null);
    setPreviewError(null);
    if (report || !objective || objective.breakthroughOnly || chosen.length === 0) return;
    let active = true;
    fetchBattlePreview(hexId, chosen, supportHq).then(
      (response) => {
        if (active && response.revision === revision) setOdds(response.odds);
      },
      (error: { message?: string }) => {
        if (active) setPreviewError(error?.message ?? String(error));
      },
    );
    return () => {
      active = false;
    };
  }, [chosen, hexId, objective, report, revision, supportHq]);

  // An advance decision must be made before the dialog can close.
  const closable = !pending;
  useEffect(() => {
    if (!closable) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [closable, onClose]);

  // ---- rows
  const defenderRows: ForceRowProps[] = [];
  const attackerRows: ForceRowProps[] = [];
  const supportRows: ForceRowProps[] = [];
  const activeAttackers = new Set<string>();

  if (!report) {
    const eligible = objective?.eligibleUnitIds ?? [];
    for (const unit of units) {
      if (hexOf(unit) !== hexId || unit.sideId === combat.sideId) continue;
      const entry = odds?.defenders.find((candidate) => candidate.unitId === unit.id);
      const step = unit.steps[unit.strengthStepIndex];
      defenderRows.push({
        unitId: unit.id,
        name: unit.name,
        sideId: unit.sideId,
        anchor: "right",
        active: true,
        detail: odds && !entry ? "Adds no Defense Strength" : strengthDetail(entry, null),
        value: entry ? strength(entry.adjusted64ths) : odds ? "0" : String(step?.defense ?? "—"),
      });
    }
    for (const id of eligible) {
      const unit = unitById.get(id);
      const checked = chosen.includes(id);
      if (checked) activeAttackers.add(id);
      const entry = odds?.attackers.find((candidate) => candidate.unitId === id);
      attackerRows.push({
        unitId: id,
        name: nameOf(id),
        sideId: combat.sideId,
        anchor: "left",
        active: checked,
        detail: strengthDetail(entry, hexOf(unit)),
        value: entry ? strength(entry.adjusted64ths) : String(unit?.steps[unit.strengthStepIndex]?.attack ?? "—"),
        checkbox: { checked, disabled: busy, onChange: () => setChosen((list) => toggle(list, id)) },
      });
    }
    for (const id of objective?.supportHqIds ?? []) {
      const checked = supportHq === id;
      supportRows.push({
        unitId: id,
        name: nameOf(id),
        sideId: combat.sideId,
        anchor: "left",
        active: checked,
        detail: [hexOf(unitById.get(id)), "once per phase"].filter(Boolean).join(" · "),
        value: "+1",
        checkbox: { checked, disabled: busy, onChange: () => setSupportHq((current) => (current === id ? null : id)) },
      });
    }
  } else {
    const describe = (id: string, defender: boolean) => {
      const unit = unitById.get(id);
      if (!unit) return "Eliminated";
      const at = hexOf(unit);
      const parts = [defender && at !== hexId ? `Retreated to ${at}` : at ?? "Off map"];
      if (unit.disruption) parts.push(unit.disruption === "suppressed" ? "Suppressed" : "Disrupted");
      return parts.join(" · ");
    };
    const defenderIds = new Set(report.odds?.defenders.map((entry) => entry.unitId) ?? []);
    for (const unit of units) {
      if (hexOf(unit) === hexId && unit.sideId !== combat.sideId) defenderIds.add(unit.id);
    }
    for (const id of defenderIds) {
      const unit = unitById.get(id);
      defenderRows.push({
        unitId: id,
        name: nameOf(id),
        sideId: roster.get(id)?.sideId ?? "",
        anchor: "right",
        active: Boolean(unit),
        detail: describe(id, true),
        value: unit ? String(unit.steps[unit.strengthStepIndex]?.defense ?? "—") : "✕",
      });
    }
    for (const id of report.attackingUnitIds) {
      const unit = unitById.get(id);
      const canAdvance = Boolean(pending?.eligibleUnitIds.includes(id));
      const checked = canAdvance && advancing.includes(id);
      if (unit && (!pending || checked)) activeAttackers.add(id);
      attackerRows.push({
        unitId: id,
        name: nameOf(id),
        sideId: combat.sideId,
        anchor: "left",
        active: Boolean(unit) && (!pending || checked),
        detail: describe(id, false),
        value: unit ? String(unit.steps[unit.strengthStepIndex]?.attack ?? "—") : "✕",
        checkbox: canAdvance ? { checked, disabled: busy, onChange: () => setAdvancing((list) => toggle(list, id)) } : undefined,
      });
    }
  }

  // ---- links between rows and mini-map chips
  const bodyRef = useRef<HTMLDivElement>(null);
  const [links, setLinks] = useState<Link[]>([]);
  const linksKey = useRef("");
  const relayout = useCallback(() => {
    if (!bodyRef.current) return;
    const next = measureLinks(bodyRef.current);
    const key = JSON.stringify(next);
    if (key === linksKey.current) return;
    linksKey.current = key;
    setLinks(next);
  }, []);
  useLayoutEffect(relayout);
  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    const observer = new ResizeObserver(relayout);
    observer.observe(body);
    return () => observer.disconnect();
  }, [relayout]);

  // ---- header and footer
  const terrain = hex ? (hex.city ? CITY_KIND_NAMES[hex.city.kind] : TERRAIN_NAMES[hex.terrain]) : "";
  const place = hex?.city?.name ?? hex?.town;
  const otherPending = combat.pendingAdvance && !pending ? combat.pendingAdvance : null;
  const defenseTotal = odds ? odds.totalDefense : report?.odds?.totalDefense;
  const attackTotal = odds ? odds.totalAttack : report?.odds?.totalAttack;

  let footer;
  if (pending) {
    footer = (
      <>
        <button type="button" className="battle-secondary" disabled={busy} onClick={() => onCommand({ type: "advanceAfterCombat", unitIds: [] })}>
          Stay in place
        </button>
        <button type="button" className="battle-primary" disabled={busy || advancing.length === 0} onClick={() => onCommand({ type: "advanceAfterCombat", unitIds: advancing })}>
          Advance ({advancing.length})
        </button>
      </>
    );
  } else if (report) {
    footer = <button type="button" className="battle-secondary" onClick={onClose}>Close</button>;
  } else {
    footer = (
      <>
        <button type="button" className="battle-secondary" onClick={onClose}>Cancel</button>
        <button
          type="button"
          className="battle-primary"
          disabled={busy || !objective || chosen.length === 0 || Boolean(otherPending)}
          onClick={() => onCommand({ type: "resolveBattle", hexId, unitIds: chosen, supportingHqId: supportHq })}
        >
          {objective?.breakthroughOnly ? "Advance" : "Attack"}
        </button>
      </>
    );
  }

  let centerNote: string | null = null;
  if (!report) {
    if (!objective) centerNote = "Checking attack options…";
    else if (objective.breakthroughOnly) centerNote = "No defenders remain: the committed units advance into the Breakthrough hex.";
    else if (chosen.length === 0) centerNote = "Tick at least one attacking unit.";
    else if (!odds && !previewError) centerNote = "Calculating odds…";
  } else if (pending) {
    centerNote = `The hex is clear. Choose which attackers advance${pending.conquersFreeCity ? " (advancing conquers the city)" : ""}.`;
  }

  return (
    <div className="settings-backdrop battle-backdrop" role="presentation" onMouseDown={closable ? onClose : undefined}>
      <section
        className="battle-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="battle-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="battle-header">
          <div>
            <span className="dialog-kicker">{sideName(combat.sideId)} attack · {terrain}</span>
            <h2 id="battle-title">Battle Planner at {hexId}{place ? ` · ${place}` : ""}</h2>
          </div>
          <div className="battle-tags">
            {objective?.mandatory && <span className="combat-tag mandatory">Must attack</span>}
            {objective?.breakthroughOnly && <span className="combat-tag">Breakthrough</span>}
          </div>
          {closable && <button type="button" className="dialog-close" onClick={onClose} aria-label="Close battle planner">×</button>}
        </header>

        <div className="battle-body" ref={bodyRef}>
          <div className="battle-column">
            <h3>Defenders{defenseTotal !== undefined && <span>{defenseTotal}</span>}</h3>
            <div className="force-list" onScroll={relayout}>
              {defenderRows.map((row) => <ForceRow key={row.unitId} {...row} />)}
              {defenderRows.length === 0 && <p className="empty">No defending units.</p>}
              {(odds ?? report?.odds)?.cityDefense ? (
                <div className="force-row city">
                  <span className="force-name">Free City organic defense</span>
                  <strong>{(odds ?? report?.odds)?.cityDefense}</strong>
                </div>
              ) : null}
            </div>
          </div>

          <div className="battle-center">
            <BattleMiniMap map={map} centerId={hexId} units={units} attackerSideId={combat.sideId} activeIds={activeAttackers} />
            {report && <ResultSummary report={report} nameOf={nameOf} />}
            {!report && odds && <OddsSummary odds={odds} />}
            {report?.odds && !pending && <OddsSummary odds={report.odds} />}
            {centerNote && <p className="battle-note">{centerNote}</p>}
            {previewError && <p className="battle-note warn">{previewError}</p>}
            {otherPending && <p className="battle-note warn">Decide the advance into {otherPending.hexId} first.</p>}
          </div>

          <div className="battle-column">
            <h3>{pending ? "Advance" : "Attackers"}{attackTotal !== undefined && !pending && <span>{attackTotal}</span>}</h3>
            <div className="force-list" onScroll={relayout}>
              {attackerRows.map((row) => <ForceRow key={row.unitId} {...row} />)}
              {attackerRows.length === 0 && <p className="empty">No units can attack this hex.</p>}
              {supportRows.length > 0 && (
                <>
                  <h4>Offensive Support (+1 column)</h4>
                  {supportRows.map((row) => <ForceRow key={row.unitId} {...row} />)}
                </>
              )}
            </div>
          </div>

          <svg className="battle-links" aria-hidden="true">
            {links.map((link) => (
              <g key={link.key} className={link.active ? undefined : "idle"}>
                <polyline points={link.points} className="link-shadow" />
                <polyline points={link.points} stroke={SIDE_COLORS[link.side]} className="link-line" />
                <circle cx={link.end.x} cy={link.end.y} r={3.5} fill={SIDE_COLORS[link.side]} />
              </g>
            ))}
          </svg>
        </div>

        <footer className="battle-footer">{footer}</footer>
      </section>
    </div>
  );
}
