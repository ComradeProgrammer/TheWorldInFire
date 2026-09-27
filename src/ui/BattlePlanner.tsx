import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  fetchBattlePreview,
  ODDS_COLUMNS,
  type BattleOdds,
  type BattleReport,
  type CombatObjective,
  type CombatState,
  type GameCommand,
  type StrengthModifier,
  type UnitState,
  type UnitStrength,
} from "../gameApi";
import { HexGrid, hexId as toHexId, neighborCoords } from "../map/hexGrid";
import { CITY_KIND_NAMES, TERRAIN_NAMES } from "../map/mapData";
import type { HexData, MapData } from "../map/mapTypes";
import { COLORS } from "../map/render/style";
import { sideName, unitSymbol } from "./unitFormat";

const MODIFIER_LABELS: Record<StrengthModifier, string> = {
  disrupted: "Disrupted ½",
  outOfCombatSupply: "Out of supply ½",
  armorIntoCityOrMountain: "Armor into city/mountain ½",
  minorRiver: "Minor river ¾",
  majorRiver: "Major river ½",
  softUnitCover: "Soft unit in cover ×2",
  provisionalDefense: "HQ provisional",
};

const SHIFT_LABELS = {
  terrain: "Terrain",
  flankAttack: "Flank attack",
  concentricAttack: "Concentric attack",
  surprise: "Surprise",
  offensiveSupport: "Offensive Support",
} as const;

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

/** The Objective hex and its six neighbours, with a chip for every unit in them. */
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
  const clusterIds = useMemo(() => new Set(cluster.map((hex) => hex.id)), [cluster]);
  const rivers = useMemo(
    () => map.hexsides.filter((side) =>
      clusterIds.has(side.a) && clusterIds.has(side.b)
      && side.features.some((feature) => feature === "majorRiver" || feature === "minorRiver")),
    [clusterIds, map.hexsides],
  );

  if (cluster.length === 0) return null;
  const hw = grid.halfWidth;
  const ry = grid.radiusY;
  const xs: number[] = [];
  const ys: number[] = [];
  for (const hex of cluster) {
    const corners = grid.corners(hex.row, hex.col);
    for (let i = 0; i < corners.length; i += 2) {
      xs.push(corners[i]);
      ys.push(corners[i + 1]);
    }
  }
  const pad = hw * 0.12;
  const minX = Math.min(...xs) - pad;
  const minY = Math.min(...ys) - pad;
  const viewBox = `${minX} ${minY} ${Math.max(...xs) + pad - minX} ${Math.max(...ys) + pad - minY}`;
  const points = (values: number[]) => values.join(" ");

  const chipW = hw * 0.46;
  const chipH = hw * 0.34;
  const gap = hw * 0.06;

  return (
    <svg className="battle-minimap" viewBox={viewBox} role="img" aria-label={`Hex ${centerId} and its neighbours`}>
      {cluster.map((hex) => {
        const { x, y } = grid.center(hex.row, hex.col);
        const fill = hex.city
          ? cssColor(COLORS[`${hex.city.kind}City`])
          : cssColor(COLORS[hex.terrain]);
        const here = units.filter((unit) => hexOf(unit) === hex.id);
        const perRow = Math.min(here.length, 3);
        const rows = Math.ceil(here.length / 3);
        const top = y - (rows * chipH + (rows - 1) * gap) / 2 + hw * 0.12;
        return (
          <g key={hex.id}>
            <polygon points={points(grid.corners(hex.row, hex.col))} fill={fill} stroke="#5fb7d6" strokeOpacity={0.5} strokeWidth={hw * 0.03} />
            <text x={x} y={y - ry * 0.6} className="minimap-hex-id" fontSize={hw * 0.2}>{hex.id}</text>
            {(hex.city?.name ?? hex.town) && (
              <text x={x} y={y + ry * 0.74} className="minimap-place" fontSize={hw * 0.16}>{hex.city?.name ?? hex.town}</text>
            )}
            {here.map((unit, index) => {
              const rowIndex = Math.floor(index / 3);
              const inRow = rowIndex === rows - 1 ? here.length - rowIndex * 3 : perRow;
              const left = x - (inRow * chipW + (inRow - 1) * gap) / 2 + (index % 3) * (chipW + gap);
              const chipTop = top + rowIndex * (chipH + gap);
              const dim = unit.sideId === attackerSideId && !activeIds.has(unit.id);
              return (
                <g key={unit.id} opacity={dim ? 0.45 : 1}>
                  <rect
                    data-chip={unit.id}
                    x={left}
                    y={chipTop}
                    width={chipW}
                    height={chipH}
                    rx={hw * 0.04}
                    fill={SIDE_COLORS[tone(unit.sideId)]}
                    stroke="#05080c"
                    strokeWidth={hw * 0.025}
                  />
                  <text x={left + chipW / 2} y={chipTop + chipH / 2} className="minimap-chip" fontSize={hw * 0.2}>{unitSymbol(unit)}</text>
                </g>
              );
            })}
          </g>
        );
      })}
      {rivers.map((side) => {
        const a = cluster.find((hex) => hex.id === side.a)!;
        const b = cluster.find((hex) => hex.id === side.b)!;
        const edge = grid.sharedSide(a, b);
        if (!edge) return null;
        const major = side.features.includes("majorRiver");
        return (
          <line
            key={`${side.a}-${side.b}`}
            x1={edge[0]}
            y1={edge[1]}
            x2={edge[2]}
            y2={edge[3]}
            stroke={cssColor(major ? COLORS.majorRiver : COLORS.minorRiver)}
            strokeWidth={hw * (major ? 0.1 : 0.06)}
            strokeLinecap="round"
          />
        );
      })}
      {(() => {
        const center = cluster[0];
        return (
          <polygon
            points={points(grid.corners(center.row, center.col, 0.94))}
            fill="none"
            stroke="#ff3b30"
            strokeWidth={hw * 0.07}
            strokeLinejoin="round"
          />
        );
      })()}
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
  const parts = entry?.modifiers.map((modifier) => MODIFIER_LABELS[modifier]) ?? [];
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
          {odds.shifts.map((shift) => `${SHIFT_LABELS[shift.reason]} ${signed(shift.shift)}`).join(" · ")}
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
    const end = { x: c.left + c.width / 2 - origin.left, y: c.top + c.height / 2 - origin.top };
    links.push({
      key: `${row.dataset.anchor}:${row.dataset.link}`,
      points: `${x},${y - origin.top} ${elbow},${y - origin.top} ${end.x},${end.y}`,
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
