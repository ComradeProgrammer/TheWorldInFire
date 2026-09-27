import { useEffect, useState } from "react";
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
} from "../gameApi";
import type { HexData } from "../map/mapTypes";

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

function signed(value: number): string {
  return value >= 0 ? `+${value}` : `${value}`;
}

function strength(sixtyFourths: number): string {
  const value = sixtyFourths / 64;
  return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(/0$/, "");
}

function unitName(units: UnitState[], id: string): string {
  return units.find((unit) => unit.id === id)?.name ?? "Eliminated unit";
}

function OddsView({ odds, units }: { odds: BattleOdds; units: UnitState[] }) {
  const rows = (entries: BattleOdds["attackers"]) =>
    entries.map((entry) => (
      <div key={entry.unitId} className="strength-row">
        <span>{unitName(units, entry.unitId)}</span>
        <small>{entry.modifiers.map((modifier) => MODIFIER_LABELS[modifier]).join(" · ")}</small>
        <strong>{strength(entry.adjusted64ths)}</strong>
      </div>
    ));
  return (
    <div className="battle-odds">
      <div className="odds-headline">
        <span><b>{odds.totalAttack}</b> vs <b>{odds.totalDefense}</b></span>
        <span>{ODDS_COLUMNS[odds.basicColumn]}{odds.netShift !== 0 && ` ${signed(odds.netShift)}`}</span>
        <strong>{odds.finalOdds}</strong>
      </div>
      <h4>Attack</h4>
      {rows(odds.attackers)}
      <h4>Defense</h4>
      {rows(odds.defenders)}
      {odds.cityDefense > 0 && (
        <div className="strength-row">
          <span>Free City organic defense</span>
          <small />
          <strong>{odds.cityDefense}</strong>
        </div>
      )}
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

function ReportView({ report, units }: { report: BattleReport; units: UnitState[] }) {
  if (!report.result || report.dieRoll === null) {
    return <p className="battle-report">Advance into the Breakthrough hex.</p>;
  }
  return (
    <div className="battle-report">
      <p>
        {report.odds?.finalOdds} · rolled <b>{report.dieRoll}</b> → <strong>{report.result.code}</strong>
      </p>
      {report.supportingHqId && <p className="support-note">Offensive Support: {unitName(units, report.supportingHqId)}</p>}
      {report.counterattacks.map((roll, index) => (
        <p key={index} className="counterattack">
          Counterattack on {unitName(units, roll.targetUnitId)}: {roll.dieRoll} → {roll.disrupted ? "Disrupted" : "no effect"}
        </p>
      ))}
    </div>
  );
}

export interface CombatPanelProps {
  hex: HexData;
  units: UnitState[];
  combat: CombatState;
  /** Core-provided objective for this hex, when it may be attacked now. */
  objective: CombatObjective | undefined;
  revision: number;
  busy: boolean;
  onCommand(command: GameCommand): void;
}

/** Combat Phase controls for the selected hex. The core validates every order. */
export function CombatPanel({ hex, units, combat, objective, revision, busy, onCommand }: CombatPanelProps) {
  const eligible = objective?.eligibleUnitIds ?? [];
  const [chosen, setChosen] = useState<string[]>(eligible);
  const [odds, setOdds] = useState<BattleOdds | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const eligibleKey = eligible.join(",");
  useEffect(() => setChosen(eligibleKey ? eligibleKey.split(",") : []), [hex.id, eligibleKey]);
  // Offensive Support is a once-per-phase choice, so it starts unselected.
  const [supportHq, setSupportHq] = useState<string | null>(null);
  const supportKey = objective?.supportHqIds.join(",") ?? "";
  useEffect(() => setSupportHq(null), [hex.id, supportKey]);

  useEffect(() => {
    setOdds(null);
    setPreviewError(null);
    if (!objective || objective.breakthroughOnly || chosen.length === 0) return;
    let active = true;
    fetchBattlePreview(hex.id, chosen, supportHq).then(
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
  }, [chosen, hex.id, objective, revision, supportHq]);

  const pending = combat.pendingAdvance?.hexId === hex.id ? combat.pendingAdvance : null;
  const [advancing, setAdvancing] = useState<string[]>([]);
  const pendingKey = pending?.eligibleUnitIds.join(",") ?? "";
  useEffect(() => setAdvancing(pendingKey ? pendingKey.split(",") : []), [pendingKey]);

  const battles = combat.battles.filter((battle) => battle.hexId === hex.id);
  const otherPending = combat.pendingAdvance && !pending ? combat.pendingAdvance : null;
  const toggle = (list: string[], id: string) => (list.includes(id) ? list.filter((entry) => entry !== id) : [...list, id]);

  if (!objective && !pending && battles.length === 0) return null;

  return (
    <section className="combat-panel">
      <header>
        <h3>Battle</h3>
        {objective?.mandatory && <span className="combat-tag mandatory">Must attack</span>}
        {objective?.breakthroughOnly && <span className="combat-tag">Breakthrough</span>}
      </header>

      {battles.map((battle) => <ReportView key={battle.id} report={battle} units={units} />)}

      {pending && (
        <div className="advance-choice">
          <h4>Advance into {hex.id}{pending.conquersFreeCity ? " (conquers the city)" : ""}</h4>
          {pending.eligibleUnitIds.map((id) => (
            <label key={id} className="strike-target">
              <input type="checkbox" checked={advancing.includes(id)} disabled={busy} onChange={() => setAdvancing((list) => toggle(list, id))} />
              <span>{unitName(units, id)}</span>
              <small />
              <strong />
            </label>
          ))}
          <div className="strike-buttons">
            <button type="button" disabled={busy || advancing.length === 0} onClick={() => onCommand({ type: "advanceAfterCombat", unitIds: advancing })}>
              Advance ({advancing.length})
            </button>
            <button type="button" disabled={busy} onClick={() => onCommand({ type: "advanceAfterCombat", unitIds: [] })}>
              Stay in place
            </button>
          </div>
        </div>
      )}

      {objective && (
        <div className="attack-setup">
          <h4>Attacking units</h4>
          {eligible.map((id) => (
            <label key={id} className="strike-target">
              <input type="checkbox" checked={chosen.includes(id)} disabled={busy} onChange={() => setChosen((list) => toggle(list, id))} />
              <span>{unitName(units, id)}</span>
              <small />
              <strong />
            </label>
          ))}
          {objective.supportHqIds.length > 0 && (
            <>
              <h4>Offensive Support (+1 column)</h4>
              {objective.supportHqIds.map((id) => (
                <label key={id} className="strike-target">
                  <input
                    type="checkbox"
                    checked={supportHq === id}
                    disabled={busy}
                    onChange={() => setSupportHq((current) => (current === id ? null : id))}
                  />
                  <span>{unitName(units, id)}</span>
                  <small>once per phase</small>
                  <strong />
                </label>
              ))}
            </>
          )}
          {objective.breakthroughOnly && <p className="strike-hint">No defenders remain: the committed units may advance into the hex.</p>}
          {odds && <OddsView odds={odds} units={units} />}
          {previewError && <p className="movement-hint warn">{previewError}</p>}
          {otherPending && <p className="movement-hint warn">Decide the advance into {otherPending.hexId} first.</p>}
          <div className="strike-buttons single">
            <button
              type="button"
              disabled={busy || chosen.length === 0 || Boolean(combat.pendingAdvance)}
              onClick={() => onCommand({ type: "resolveBattle", hexId: hex.id, unitIds: chosen, supportingHqId: supportHq })}
            >
              {objective.breakthroughOnly ? "Enter Breakthrough hex" : "Resolve battle"}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
