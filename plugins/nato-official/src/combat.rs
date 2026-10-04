//! Combat Phase (rule 25), simplified so that only the Attacker makes choices.
//!
//! The Attacker picks each Objective hex, the committed units, and whether to
//! advance. Decisions the rules give to the Defender are taken automatically:
//! which defending steps are lost, retreat routes, Counterattack targets, and
//! which attacking step absorbs an A1 result. Attack Helicopters, Reaction,
//! NATO Defensive Strikes, Exploitation, Assaults, and Coordination are not
//! implemented.

use std::cmp::Reverse;
use std::collections::HashMap;

use crate::airspace::hex_distance;
use crate::error::RuleError;
use crate::event::GameEvent;
use crate::filters::{self, ColumnShiftsInput, CombatResultInput, UnitStrengthInput};
use crate::model::{
    BattleOdds, BattleReport, CityKind, ColumnShift, ColumnShiftReason, CombatObjective,
    CombatOptions, CombatResult, CombatState, CounterattackRoll, HexId, HexsideFeature, MapHex,
    PendingAdvance, PhaseActor, SideId, StrengthModifier, SupplyStatus, Terrain, UnitId,
    UnitLocation, UnitState, UnitStrength, ODDS_COLUMNS,
};
use crate::movement::MovementContext;
use crate::rules::Rules;
use crate::supply::{los_distances, support_range};

/// Combat Results Table as printed on the map: rows are die rolls 1-6,
/// columns follow [`ODDS_COLUMNS`].
const CRT: [[&str; 13]; 6] = [
    [
        "A1*/-", "A1*/-", "A1*/-", "A1*/-", "A1/-", "A1/-", "A1/R1", "A1/R1", "A1/D1R1",
        "-/CAD1R1", "-/D1R1", "-/D1R2", "-/D1*R2",
    ],
    [
        "A1*/-", "A1*/-", "A1*/-", "A1*/-", "A1/-", "A1/R1", "A1/R1", "A1/D1R1", "-/CAD1R1",
        "-/D1R1", "-/D1R2", "-/D1*R2", "A1/D2*R2",
    ],
    [
        "A1*/-", "A1*/-", "A1*/-", "A1/-", "A1/R1", "A1/R1", "A1/D1R1", "-/CAD1R1", "-/D1R1",
        "-/D1R2", "-/D1*R2", "A1/D2*R2", "-/D2*R2",
    ],
    [
        "A1*/-", "A1*/-", "A1/-", "A1/-", "A1/R1", "A1/D1R1", "-/CAD1R1", "-/D1R1", "-/D1R2",
        "-/D1*R2", "A1/D2*R2", "-/D2*R2", "-/D2*R2",
    ],
    [
        "A1*/-", "A1/-", "A1/-", "A1/R1", "A1/D1R1", "-/CAD1R1", "-/D1R1", "-/D1R2", "-/D1*R2",
        "A1/D2*R2", "-/D2*R2", "-/D2*R2", "-/D2*R2",
    ],
    [
        "A1/-", "A1/-", "A1/R1", "A1/D1R1", "-/CAD1R1", "-/D1R1", "-/D1R2", "-/D1*R2", "A1/D2*R2",
        "-/D2*R2", "-/D2*R2", "-/D2*R2", "-/D2*R2",
    ],
];

/// Index of the 1:1 column.
const ONE_TO_ONE: usize = 3;
/// Strength arithmetic keeps fractions exact in sixty-fourths.
const SCALE: u32 = 64;

/// Parses a printed CRT cell such as `A1*/-` or `-/CAD1R1` (25.6).
pub fn parse_result(code: &str) -> CombatResult {
    let (attacker, defender) = code.split_once('/').unwrap_or((code, "-"));
    let digit_after = |text: &str, marker: char| {
        text.find(marker)
            .and_then(|index| text[index + 1..].chars().next())
            .and_then(|c| c.to_digit(10))
            .unwrap_or(0) as u8
    };
    CombatResult {
        code: code.to_owned(),
        attacker_steps: digit_after(attacker, 'A'),
        attacker_disrupted: attacker.contains('*'),
        counterattack: defender.starts_with("CA"),
        defender_steps: digit_after(defender, 'D'),
        defender_disrupted: defender.contains('*'),
        retreat: digit_after(defender, 'R'),
    }
}

/// Returns the lowest Counterattack roll that disrupts a target for the defending step's nationality.
fn counterattack_threshold(unit: &UnitState) -> u8 {
    // Counterattack Table: the lowest roll that Disrupts the target, by the
    // nationality of the Counterattacking step.
    match (
        unit.definition.side_id.0.as_str(),
        unit.definition.nation_id.0.as_str(),
    ) {
        ("nato", "westGermany") => 3,
        ("nato", _) => 4,
        (_, "sovietUnion") => 4,
        _ => 5,
    }
}

/// Armored units (tank and armored cavalry) are halved attacking cities and mountains.
fn is_armored(unit: &UnitState) -> bool {
    let kind = unit.definition.unit_type_id.0.to_ascii_lowercase();
    ["tank", "armored", "cavalry"]
        .iter()
        .any(|key| kind.contains(key))
}

/// Checks whether a unit is a non-headquarters maneuver unit without the hard trait.
fn is_soft_maneuver(unit: &UnitState) -> bool {
    !unit.is_hard() && !unit.is_headquarters()
}

/// Checks whether a unit currently occupies the specified map hex.
fn in_hex(unit: &UnitState, hex_id: &HexId) -> bool {
    matches!(&unit.location, UnitLocation::Hex { hex_id: at } if at == hex_id)
}

/// Returns the active step's printed attack strength, or zero if no step remains.
fn current_attack(unit: &UnitState) -> u16 {
    unit.current_step().map_or(0, |step| step.attack)
}

/// Returns the active step's printed defense strength, or zero if no step remains.
fn current_defense(unit: &UnitState) -> u16 {
    unit.current_step().map_or(0, |step| step.defense)
}

/// Checks whether the unit is explicitly marked out of combat supply.
fn out_of_combat_supply(unit: &UnitState) -> bool {
    unit.supply.combat == Some(SupplyStatus::OutOfSupply)
}

/// Column shift for the Objective hex's primary terrain (25.3.4.1).
fn terrain_shift(hex: &MapHex) -> i8 {
    match (&hex.city, hex.terrain) {
        (Some(city), _) if matches!(city.kind, CityKind::Major | CityKind::Key) => -2,
        (Some(_), _) => -1,
        (None, Terrain::Forest | Terrain::Rough | Terrain::Mountain) => -1,
        _ => 0,
    }
}

/// Retreat hexes the Objective hex's primary terrain cancels (25.7.2).
fn retreat_reduction(hex: &MapHex) -> u8 {
    match (&hex.city, hex.terrain) {
        (Some(city), _) if matches!(city.kind, CityKind::Major | CityKind::Key) => 2,
        (Some(_), _) => 1,
        (None, Terrain::Mountain) => 2,
        (None, Terrain::Rough) => 1,
        _ => 0,
    }
}

/// A unit may always ignore the second retreat hex after entering one of these (25.7.3).
fn stops_second_retreat_hex(hex: &MapHex) -> bool {
    retreat_reduction(hex) > 0
}

/// Soft units double their Defense in Forest, Rough, Mountain, or a City (25.3.2.2).
fn gives_soft_cover(hex: &MapHex) -> bool {
    hex.city.is_some()
        || matches!(
            hex.terrain,
            Terrain::Forest | Terrain::Rough | Terrain::Mountain
        )
}

/// Maps total strengths to a bounded odds column, rounding in the defender's favor.
fn column_for(total_attack: u16, total_defense: u16) -> usize {
    if total_attack == 0 {
        return 0;
    }
    let defense = total_defense.max(1);
    if total_attack >= defense {
        let ratio = usize::from(total_attack / defense).min(10);
        ONE_TO_ONE + ratio - 1
    } else {
        // Odds below 1:1 round in the Defender's favour: 5 against 8 is 1:2.
        let ratio = usize::from(defense.div_ceil(total_attack)).min(4);
        ONE_TO_ONE + 1 - ratio
    }
}

/// Returns the Flank or Concentric odds shift when attackers surround every adjacent hex (25.3.4.2-3).
fn envelopment_shift(defender_context: &MovementContext, hex_id: &HexId) -> Option<ColumnShift> {
    // 25.3.4.2-3: Flank (+1) or Concentric (+2) when every adjacent hex holds
    // an attacking unit or lies in the Attacker's ZOC.
    let hex = defender_context.hexes.get(hex_id.0.as_str())?;
    let row = i32::from(hex.row);
    let col = i32::from(hex.col);
    let diagonal = if row % 2 == 0 {
        [
            (row - 1, col),
            (row - 1, col + 1),
            (row + 1, col),
            (row + 1, col + 1),
        ]
    } else {
        [
            (row - 1, col - 1),
            (row - 1, col),
            (row + 1, col - 1),
            (row + 1, col),
        ]
    };
    let positions: Vec<String> = [(row, col - 1), (row, col + 1)]
        .into_iter()
        .chain(diagonal)
        .map(|(r, c)| format!("{r:02}{c:02}"))
        .collect();
    // A map edge never counts as surrounded (Designer's Note to 25.3.4.3).
    let surrounded = positions.iter().all(|id| {
        defender_context.hexes.contains_key(id.as_str())
            && (defender_context.enemy_occupied.contains(id.as_str())
                || defender_context.in_enemy_zoc(id))
    });
    if !surrounded {
        return None;
    }
    let supported = positions.iter().any(|id| {
        defender_context.hexes.contains_key(id.as_str())
            && !defender_context.prohibited_hexside(&hex_id.0, id)
            && (defender_context.friendly_occupied.contains(id.as_str())
                || defender_context.friendly_free_cities.contains(id.as_str()))
    });
    Some(if supported {
        ColumnShift {
            reason: ColumnShiftReason::FlankAttack,
            shift: 1,
        }
    } else {
        ColumnShift {
            reason: ColumnShiftReason::ConcentricAttack,
            shift: 2,
        }
    })
}

/// One candidate retreat route and the reasons it is good or bad (25.7.4).
struct RetreatRoute {
    /// Retreat hexes in traversal order, excluding the original defended hex.
    path: Vec<HexId>,
    /// Hexes still owed, each costing a step.
    unfulfilled: u8,
    /// Unnegated EZOC hexes entered, each costing a step.
    ezoc_hexes: u8,
}

/// Which hexes an Objective could be; `None` means it is not a legal objective.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ObjectiveKind {
    Defended,
    BreakthroughOnly,
}

impl Rules {
    // ------------------------------------------------------------------ phase hooks

    /// Initializes empty battle records and combat restrictions for the acting side.
    pub(crate) fn start_combat(&mut self) {
        let Some(PhaseActor::Side { side_id }) = self.current_step().map(|step| step.actor.clone())
        else {
            return;
        };
        self.combat = Some(CombatState {
            side_id,
            battles: Vec::new(),
            attacked_unit_ids: Vec::new(),
            attacked_hex_ids: Vec::new(),
            engaged_unit_ids: Vec::new(),
            supporting_hq_ids: Vec::new(),
            pending_advance: None,
        });
    }

    /// 25.0: the Warsaw Pact must attack every Battle Marker hex it still can.
    pub(crate) fn check_combat_can_end(&self) -> Result<(), RuleError> {
        let remaining = self.combat_options()?.mandatory_remaining;
        if remaining.is_empty() {
            Ok(())
        } else {
            Err(RuleError::new(
                "mandatoryAttacksRemaining",
                format!(
                    "Every marked objective must be attacked: {}",
                    remaining
                        .iter()
                        .map(|hex| hex.0.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ))
        }
    }

    /// Declines any pending advance; Engaged markers are removed with the phase (25.6.5).
    pub(crate) fn finish_combat(&mut self, events: &mut Vec<GameEvent>) {
        if self
            .combat
            .as_ref()
            .is_some_and(|combat| combat.pending_advance.is_some())
        {
            self.finish_advance(Vec::new(), events);
        }
        self.combat = None;
    }

    // ------------------------------------------------------------------ preview

    /// Lists currently legal combat objectives and their eligible attackers.
    ///
    /// The preview includes available support HQs, breakthrough-only objectives,
    /// and remaining mandatory Warsaw Pact objectives. Previously attacked hexes
    /// and units are excluded according to the active phase's restrictions.
    ///
    /// # Returns
    ///
    /// Read-only [`CombatOptions`] calculated from the current game state.
    ///
    /// # Errors
    ///
    /// Returns a [`RuleError`] if the game has ended, the current phase is not a
    /// single-side Combat Phase, or the combat state has not been initialized.
    pub fn combat_options(&self) -> Result<CombatOptions, RuleError> {
        let side_id = self.combat_side()?;
        let combat = self.combat_state()?;
        let context = MovementContext::new(self, &side_id);
        let marked: Vec<HexId> = if side_id.0 == "warsawPact" {
            self.battle_plan
                .as_ref()
                .map(|plan| plan.attack_targets.clone())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut objectives = Vec::new();
        let mut mandatory_remaining = Vec::new();
        for hex in &self.scenario.map.hexes {
            let Some(kind) = self.objective_kind(&side_id, &hex.id) else {
                continue;
            };
            if combat.attacked_hex_ids.contains(&hex.id) {
                continue;
            }
            // 25.1.1: the WP attacks only Battle Marker or Breakthrough Marker hexes.
            let mandatory = marked.contains(&hex.id);
            if side_id.0 == "warsawPact" && !mandatory && kind != ObjectiveKind::BreakthroughOnly {
                continue;
            }
            let eligible: Vec<UnitId> = self
                .units
                .values()
                .filter(|unit| self.can_attack(&context, &side_id, unit, &hex.id))
                .map(|unit| unit.id().clone())
                .collect();
            if eligible.is_empty() {
                continue;
            }
            let can_resolve = kind == ObjectiveKind::BreakthroughOnly
                || eligible
                    .iter()
                    .any(|id| current_attack(&self.units[id]) > 0);
            if mandatory && can_resolve {
                mandatory_remaining.push(hex.id.clone());
            }
            let eligible_units: Vec<&UnitState> =
                eligible.iter().map(|id| &self.units[id]).collect();
            let support_hq_ids = if kind == ObjectiveKind::Defended {
                self.available_support(&side_id, &eligible_units)
            } else {
                Vec::new()
            };
            objectives.push(CombatObjective {
                support_hq_ids,
                hex_id: hex.id.clone(),
                eligible_unit_ids: eligible,
                mandatory,
                breakthrough_only: kind == ObjectiveKind::BreakthroughOnly,
            });
        }
        Ok(CombatOptions {
            objectives,
            mandatory_remaining,
        })
    }

    /// Calculates strengths, column shifts, odds, and possible results for a battle.
    ///
    /// Validates the proposed attackers and optional support without rolling dice
    /// or committing units. The battle command validates the current state again.
    ///
    /// # Parameters
    ///
    /// - `hex_id`: Legal objective hex containing defenders or an enemy Free City.
    /// - `unit_ids`: Nonempty list of distinct eligible attacking units; at least
    ///   one must have positive attack strength for a defended objective.
    /// - `supporting_hq_id`: Optional eligible HQ to include for Offensive Support;
    ///   `None` previews the battle without HQ support.
    ///
    /// # Returns
    ///
    /// [`BattleOdds`] including adjusted strengths and the six possible results.
    ///
    /// # Errors
    ///
    /// Returns a [`RuleError`] for an unavailable Combat Phase, illegal objective,
    /// ineligible or duplicate attackers, insufficient attack strength, or invalid
    /// support. Returns `breakthroughAdvance` for an advance-only objective,
    /// which has no combat odds.
    pub fn battle_preview(
        &self,
        hex_id: &HexId,
        unit_ids: &[UnitId],
        supporting_hq_id: Option<&UnitId>,
    ) -> Result<BattleOdds, RuleError> {
        let side_id = self.combat_side()?;
        let attackers = self.validate_attack(hex_id, unit_ids)?;
        if let Some(hq_id) = supporting_hq_id {
            self.validate_support(&side_id, hq_id, &attackers)?;
        }
        match self.objective_kind(&side_id, hex_id) {
            Some(ObjectiveKind::Defended) => {
                self.battle_odds(&side_id, hex_id, &attackers, supporting_hq_id.is_some())
            }
            _ => Err(RuleError::new(
                "breakthroughAdvance",
                "The hex holds only a Breakthrough Marker: attacking it is an advance with no odds",
            )),
        }
    }

    // ------------------------------------------------------------------ commands

    /// Validates and resolves an attack, applying losses, retreats, support, and advance choices.
    pub(super) fn resolve_battle(
        &mut self,
        hex_id: HexId,
        unit_ids: Vec<UnitId>,
        supporting_hq_id: Option<UnitId>,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.combat_side()?;
        if self.combat_state()?.pending_advance.is_some() {
            return Err(RuleError::new(
                "advancePending",
                "Decide whether to advance after the previous battle first",
            ));
        }
        let attackers: Vec<UnitId> = self
            .validate_attack(&hex_id, &unit_ids)?
            .iter()
            .map(|unit| unit.id().clone())
            .collect();
        let kind = self
            .objective_kind(&side_id, &hex_id)
            .expect("validated objective");
        if let Some(hq_id) = &supporting_hq_id {
            if kind == ObjectiveKind::BreakthroughOnly {
                return Err(RuleError::new(
                    "supportUnavailable",
                    "An advance into an empty Breakthrough hex needs no Offensive Support",
                ));
            }
            let units: Vec<&UnitState> = attackers.iter().map(|id| &self.units[id]).collect();
            self.validate_support(&side_id, hq_id, &units)?;
        }
        let mut events = Vec::new();
        let combat = self.combat.as_mut().expect("checked combat state");
        combat.attacked_hex_ids.push(hex_id.clone());
        combat.attacked_unit_ids.extend(attackers.iter().cloned());
        combat
            .supporting_hq_ids
            .extend(supporting_hq_id.iter().cloned());
        let battle_id = combat.battles.len() as u32 + 1;

        // 25.9.1: an empty Breakthrough hex is taken by advancing, with no roll.
        if kind == ObjectiveKind::BreakthroughOnly {
            let report = BattleReport {
                id: battle_id,
                hex_id: hex_id.clone(),
                attacking_unit_ids: attackers.clone(),
                odds: None,
                die_roll: None,
                result: None,
                counterattacks: Vec::new(),
                supporting_hq_id: None,
            };
            self.record_battle(&side_id, report, &mut events);
            self.offer_advance(battle_id, &hex_id, &attackers, false, &mut events);
            return Ok(events);
        }

        let attacker_units: Vec<&UnitState> = attackers.iter().map(|id| &self.units[id]).collect();
        let odds = self.battle_odds(
            &side_id,
            &hex_id,
            &attacker_units,
            supporting_hq_id.is_some(),
        )?;
        let die_roll = self.dice.d6();
        let result = self.combat_result(&side_id, &hex_id, odds.final_column, die_roll)?;

        let defenders: Vec<UnitId> = self
            .units
            .values()
            .filter(|unit| unit.definition.side_id != side_id && in_hex(unit, &hex_id))
            .map(|unit| unit.id().clone())
            .collect();
        let hex = self.map_hex(&hex_id)?;
        let enemy_free_city = self.is_enemy_free_city(&side_id, &hex_id);
        let city_alone = enemy_free_city && defenders.is_empty();

        // 25.6.1: Counterattacks are rolled before any other result is applied.
        let mut consequences = Vec::new();
        let counterattacks = if result.counterattack {
            self.counterattack(&defenders, &attackers, &mut consequences)
        } else {
            Vec::new()
        };

        // Defender Results Segment (25.6.1.1).
        let defender_absorbed_all = self.apply_defender_losses(
            &hex_id,
            &defenders,
            result.defender_steps,
            &mut consequences,
        );
        // 30.4, 25.6.3.2: a Free City defending alone loses its Organic Defense to any step loss.
        let city_broken = city_alone && result.defender_steps > 0;
        let net_retreat = result.retreat.saturating_sub(retreat_reduction(&hex));
        if net_retreat > 0 {
            self.retreat_defenders(
                &side_id,
                &hex_id,
                &attackers,
                net_retreat,
                &mut consequences,
            );
        }
        let surviving_defenders: Vec<UnitId> = defenders
            .iter()
            .filter(|id| self.units.contains_key(*id))
            .cloned()
            .collect();
        if result.defender_disrupted {
            for id in &surviving_defenders {
                self.disrupt(id, &mut consequences);
            }
        }
        if let Some(combat) = self.combat.as_mut() {
            combat
                .engaged_unit_ids
                .extend(surviving_defenders.iter().cloned());
        }

        // Attacker Results Segment (25.6.1.2). 25.6.3.1: the Attacker may ignore
        // an A1 when the Defender could not absorb all of its losses.
        if result.attacker_steps > 0 && defender_absorbed_all {
            self.apply_attacker_loss(&attackers, &mut consequences);
        }
        if result.attacker_disrupted {
            for id in &attackers {
                self.disrupt(id, &mut consequences);
            }
        }

        let report = BattleReport {
            id: battle_id,
            hex_id: hex_id.clone(),
            attacking_unit_ids: attackers.clone(),
            odds: Some(odds),
            die_roll: Some(die_roll),
            result: Some(result),
            counterattacks,
            supporting_hq_id,
        };
        self.record_battle(&side_id, report, &mut events);
        events.extend(consequences);

        // 25.8: the Attacker may advance when no defender remains and no enemy
        // Free City still holds the hex on its own.
        let cleared = !self
            .units
            .values()
            .any(|unit| unit.definition.side_id != side_id && in_hex(unit, &hex_id));
        let city_holds = city_alone && !city_broken;
        let survivors: Vec<UnitId> = attackers
            .iter()
            .filter(|id| self.units.contains_key(*id))
            .cloned()
            .collect();
        if cleared && !city_holds && !survivors.is_empty() {
            self.offer_advance(battle_id, &hex_id, &survivors, enemy_free_city, &mut events);
        } else if cleared && !city_holds {
            self.place_breakthrough(&hex_id, &mut events);
        }
        Ok(events)
    }

    /// Validates the chosen surviving attackers and stacking before completing a pending advance.
    pub(super) fn advance_after_combat(
        &mut self,
        unit_ids: Vec<UnitId>,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.combat_side()?;
        let pending = self
            .combat_state()?
            .pending_advance
            .clone()
            .ok_or_else(|| {
                RuleError::new("noAdvancePending", "No battle is waiting for an advance")
            })?;
        for id in &unit_ids {
            if !pending.eligible_unit_ids.contains(id) || !self.units.contains_key(id) {
                return Err(RuleError::new(
                    "unitCannotAdvance",
                    "Only surviving units that fought the battle may advance",
                ));
            }
        }
        // 25.8.1: Consolidation observes the normal stacking limit.
        let steps: u16 = unit_ids
            .iter()
            .map(|id| &self.units[id])
            .filter(|unit| !unit.is_headquarters())
            .map(UnitState::step_count)
            .sum();
        let already: u16 = self
            .units
            .values()
            .filter(|unit| {
                unit.definition.side_id == side_id
                    && in_hex(unit, &pending.hex_id)
                    && !unit.is_headquarters()
            })
            .map(UnitState::step_count)
            .sum();
        let limit = self.scenario.battle_planning_rules.maneuver_stacking_limit;
        if steps + already > limit {
            return Err(RuleError::new(
                "stackingLimitExceeded",
                format!("Advancing units may not exceed the {limit}-step stacking limit"),
            ));
        }
        let mut events = Vec::new();
        self.finish_advance(unit_ids, &mut events);
        Ok(events)
    }

    // ------------------------------------------------------------------ battle mechanics

    /// Stores a battle report in the active combat state and emits its resolution event.
    fn record_battle(
        &mut self,
        side_id: &SideId,
        report: BattleReport,
        events: &mut Vec<GameEvent>,
    ) {
        if let Some(combat) = self.combat.as_mut() {
            combat.battles.push(report.clone());
        }
        events.push(GameEvent::BattleResolved {
            side_id: side_id.clone(),
            report,
        });
    }

    /// Records and emits the attacker's pending choice to advance into a cleared objective.
    fn offer_advance(
        &mut self,
        battle_id: u32,
        hex_id: &HexId,
        units: &[UnitId],
        conquers_free_city: bool,
        events: &mut Vec<GameEvent>,
    ) {
        let pending = PendingAdvance {
            battle_id,
            hex_id: hex_id.clone(),
            eligible_unit_ids: units.to_vec(),
            conquers_free_city,
        };
        if let Some(combat) = self.combat.as_mut() {
            combat.pending_advance = Some(pending.clone());
        }
        events.push(GameEvent::AdvanceOffered { pending });
    }

    /// Moves chosen attackers, takes enemy cities, and places any allowed Breakthrough Marker (25.8-25.9).
    fn finish_advance(&mut self, unit_ids: Vec<UnitId>, events: &mut Vec<GameEvent>) {
        // Moves the advancing units (possibly none), takes any enemy city, and
        // places a Breakthrough Marker in the cleared hex (25.9.2).
        let Some(combat) = self.combat.as_mut() else {
            return;
        };
        let Some(pending) = combat.pending_advance.take() else {
            return;
        };
        let side_id = combat.side_id.clone();
        for id in &unit_ids {
            if let Some(unit) = self.units.get_mut(id) {
                unit.location = UnitLocation::Hex {
                    hex_id: pending.hex_id.clone(),
                };
            }
        }
        if !unit_ids.is_empty() {
            events.push(GameEvent::UnitsAdvanced {
                side_id: side_id.clone(),
                hex_id: pending.hex_id.clone(),
                unit_ids: unit_ids.clone(),
            });
            // 30.1.1, 30.4.9: advancing into an enemy city takes it.
            if let Some(controller) = self.city_control.get(&pending.hex_id).cloned() {
                if controller != side_id {
                    self.city_control
                        .insert(pending.hex_id.clone(), side_id.clone());
                    events.push(GameEvent::CityControlChanged {
                        hex_id: pending.hex_id.clone(),
                        free: self.city_owner(&pending.hex_id) == Some(&side_id),
                        controller: side_id.clone(),
                    });
                }
            }
        }
        // 25.9.2 (2): no marker while an enemy Free City still holds the hex.
        if !self.is_enemy_free_city(&side_id, &pending.hex_id) {
            self.place_breakthrough(&pending.hex_id, events);
        }
    }

    /// Adds a Breakthrough Marker and emits its placement event unless it is already present.
    fn place_breakthrough(&mut self, hex_id: &HexId, events: &mut Vec<GameEvent>) {
        if !self.breakthrough_markers.contains(hex_id) {
            self.breakthrough_markers.push(hex_id.clone());
            events.push(GameEvent::BreakthroughMarkerPlaced {
                hex_id: hex_id.clone(),
            });
        }
    }

    /// Rolls eligible defending steps against the strongest undisrupted attackers using NATO nationality limits (25.6.2).
    fn counterattack(
        &mut self,
        defenders: &[UnitId],
        attackers: &[UnitId],
        events: &mut Vec<GameEvent>,
    ) -> Vec<CounterattackRoll> {
        // 25.6.2: each eligible defending step rolls once. Steps of one nationality
        // only (the one with the most eligible steps) Counterattack for NATO; the
        // target is the strongest attacker not yet Disrupted.
        let mut steps: Vec<UnitId> = Vec::new();
        for id in defenders {
            let unit = &self.units[id];
            let eligible = !unit.is_headquarters()
                && unit.disruption.is_none()
                && unit.train_status.is_none()
                && unit.supply.combat != Some(SupplyStatus::OutOfSupply)
                && unit.supply.movement != Some(SupplyStatus::OutOfSupply)
                && unit
                    .definition
                    .steps
                    .first()
                    .is_some_and(|step| step.attack > 0);
            if eligible {
                steps.extend(std::iter::repeat_n(
                    id.clone(),
                    usize::from(unit.step_count()),
                ));
            }
        }
        // 25.6.2.1: NATO Counterattacks with one nationality only.
        if steps
            .first()
            .is_some_and(|id| self.units[id].definition.side_id.0 == "nato")
        {
            let mut by_nation: Vec<(String, usize)> = Vec::new();
            for id in &steps {
                let nation = self.units[id].definition.nation_id.0.clone();
                match by_nation.iter_mut().find(|(name, _)| *name == nation) {
                    Some((_, count)) => *count += 1,
                    None => by_nation.push((nation, 1)),
                }
            }
            by_nation.sort_by_key(|(name, count)| (Reverse(*count), name.clone()));
            if let Some((nation, _)) = by_nation.first().cloned() {
                steps.retain(|id| self.units[id].definition.nation_id.0 == nation);
            }
        }
        let mut rolls = Vec::new();
        for id in steps {
            let Some(target) = attackers
                .iter()
                .filter(|target| self.units.contains_key(*target))
                .max_by_key(|target| {
                    let unit = &self.units[*target];
                    (
                        unit.disruption.is_none(),
                        current_attack(unit),
                        Reverse((*target).clone()),
                    )
                })
                .cloned()
            else {
                break;
            };
            let die_roll = self.dice.d6();
            let disrupted = die_roll >= counterattack_threshold(&self.units[&id]);
            if disrupted {
                self.disrupt(&target, events);
            }
            rolls.push(CounterattackRoll {
                unit_id: id,
                target_unit_id: target,
                die_roll,
                disrupted,
            });
        }
        rolls
    }

    /// Applies defender losses by unit priority and reports whether all required losses were absorbed (25.6.3).
    fn apply_defender_losses(
        &mut self,
        hex_id: &HexId,
        defenders: &[UnitId],
        steps: u8,
        events: &mut Vec<GameEvent>,
    ) -> bool {
        // 25.6.3: defending steps are lost from contributing Maneuver units first
        // (strongest first), then other Maneuver units, then HQs, which are
        // Suppressed instead of reduced. Units under a train marker alone in the
        // hex are eliminated outright (13.4.3). Returns whether every loss was absorbed.
        if !defenders.is_empty()
            && defenders
                .iter()
                .all(|id| self.units[id].train_status.is_some())
        {
            for id in defenders {
                while self.remove_step(id, hex_id, events) {}
            }
            return true;
        }
        let engaged = self
            .combat
            .as_ref()
            .map(|combat| combat.engaged_unit_ids.clone())
            .unwrap_or_default();
        let mut suppressed_hqs: Vec<UnitId> = Vec::new();
        for _ in 0..steps {
            let target = defenders
                .iter()
                .filter(|id| self.units.contains_key(*id))
                .filter(|id| !suppressed_hqs.contains(id))
                .min_by_key(|id| {
                    let unit = &self.units[*id];
                    let rank = if unit.is_headquarters() {
                        2
                    } else if unit.train_status.is_some() || engaged.contains(id) {
                        1
                    } else {
                        0
                    };
                    (rank, Reverse(current_defense(unit)), (*id).clone())
                })
                .cloned();
            let Some(target) = target else {
                return false;
            };
            if self.units[&target].is_headquarters() {
                self.disrupt(&target, events);
                suppressed_hqs.push(target);
            } else {
                self.remove_step(&target, hex_id, events);
            }
        }
        true
    }

    /// Takes one attacking step from the unit with the highest current Attack Strength (25.6.3).
    fn apply_attacker_loss(&mut self, attackers: &[UnitId], events: &mut Vec<GameEvent>) {
        // 25.6.3: the Defender picks the attacking step lost; taken automatically
        // from the attacker with the highest current Attack Strength.
        let target = attackers
            .iter()
            .filter(|id| self.units.contains_key(*id))
            .max_by_key(|id| (current_attack(&self.units[*id]), Reverse((*id).clone())))
            .cloned();
        if let Some(target) = target {
            let UnitLocation::Hex { hex_id } = self.units[&target].location.clone() else {
                return;
            };
            self.remove_step(&target, &hex_id, events);
        }
    }

    /// Retreats surviving defenders together and applies losses for enemy ZOCs and unfulfilled retreat hexes (25.7).
    fn retreat_defenders(
        &mut self,
        attacker_side: &SideId,
        objective: &HexId,
        attackers: &[UnitId],
        net_retreat: u8,
        events: &mut Vec<GameEvent>,
    ) {
        // 25.7: the surviving defenders retreat together along the best legal
        // route, losing a step for each unnegated EZOC hex entered and each hex
        // they cannot retreat. Units under a train marker are eliminated (13.4.4).
        let defenders: Vec<UnitId> = self
            .units
            .values()
            .filter(|unit| unit.definition.side_id != *attacker_side && in_hex(unit, objective))
            .map(|unit| unit.id().clone())
            .collect();
        if defenders.is_empty() {
            return;
        }
        let entrained: Vec<UnitId> = defenders
            .iter()
            .filter(|id| self.units[*id].train_status.is_some())
            .cloned()
            .collect();
        for id in &entrained {
            while self.remove_step(id, objective, events) {}
        }
        let retreating: Vec<UnitId> = defenders
            .into_iter()
            .filter(|id| self.units.contains_key(id))
            .collect();
        if retreating.is_empty() {
            return;
        }
        let defender_side = self.units[&retreating[0]].definition.side_id.clone();
        let route = self.best_retreat(
            &defender_side,
            objective,
            attackers,
            &retreating,
            net_retreat,
        );

        // The Defender chooses who pays: its weakest units first.
        let losses = route.unfulfilled + route.ezoc_hexes;
        for _ in 0..losses {
            let Some(target) = retreating
                .iter()
                .filter(|id| self.units.contains_key(*id))
                .min_by_key(|id| (current_defense(&self.units[*id]), (*id).clone()))
                .cloned()
            else {
                break;
            };
            self.remove_step(&target, objective, events);
        }
        let Some(destination) = route.path.last().cloned() else {
            return;
        };
        let survivors: Vec<UnitId> = retreating
            .into_iter()
            .filter(|id| self.units.contains_key(id))
            .collect();
        for id in &survivors {
            if let Some(unit) = self.units.get_mut(id) {
                unit.location = UnitLocation::Hex {
                    hex_id: destination.clone(),
                };
            }
            events.push(GameEvent::UnitRetreated {
                unit_id: id.clone(),
                from: objective.clone(),
                path: route.path.clone(),
            });
        }
    }

    /// Selects the legal retreat route with the lowest losses and best remaining retreat priorities (25.7.4).
    fn best_retreat(
        &self,
        defender_side: &SideId,
        objective: &HexId,
        attackers: &[UnitId],
        retreating: &[UnitId],
        net_retreat: u8,
    ) -> RetreatRoute {
        // Chooses the retreat route by the priorities of 25.7.4: fewest step
        // losses, then not adjacent to enemy units, not overstacked, avoiding
        // Mountains and Major Rivers, and finally farthest from the attackers
        // (standing in for "toward the friendly rear").
        let context = MovementContext::new(self, defender_side);
        let hexes = &context.hexes;
        let origin = hexes[objective.0.as_str()];
        let retreating_steps: u16 = retreating
            .iter()
            .map(|id| self.units[id].step_count())
            .sum();
        let attacker_hexes: Vec<&MapHex> = attackers
            .iter()
            .filter_map(|id| self.units.get(id))
            .filter_map(|unit| match &unit.location {
                UnitLocation::Hex { hex_id } => hexes.get(hex_id.0.as_str()).copied(),
                UnitLocation::StrategicReserve => None,
            })
            .collect();
        let enterable = |from: &str, to: &str, step: i32| {
            let Some(hex) = hexes.get(to) else {
                return false;
            };
            hex.terrain != Terrain::Sea
                && hex_distance(origin, hex) == step
                && !context.prohibited_hexside(from, to)
                && !context.enemy_occupied.contains(to)
                && !context.enemy_free_cities.contains(to)
                && !context.enemy_conquered_cities.contains(to)
        };

        // Every route of up to two hexes that keeps moving away from the objective.
        let mut routes: Vec<Vec<&str>> = Vec::new();
        for first in context.neighbors(&objective.0) {
            if !enterable(&objective.0, first, 1) {
                continue;
            }
            routes.push(vec![first]);
            for second in context.neighbors(first) {
                if enterable(first, second, 2) {
                    routes.push(vec![first, second]);
                }
            }
        }
        let score = |route: &Vec<&str>| {
            let ezoc = route
                .iter()
                .filter(|id| context.in_enemy_zoc(id) && !context.negates_ezoc_for_entry(id))
                .count() as u8;
            let last = hexes[route[route.len() - 1]];
            let adjacent_enemy = context
                .neighbors(&last.id.0)
                .into_iter()
                .filter(|id| context.enemy_occupied.contains(id))
                .count();
            let stacked: u16 = self
                .units
                .values()
                .filter(|unit| unit.definition.side_id == *defender_side && in_hex(unit, &last.id))
                .map(UnitState::step_count)
                .sum();
            let overstacked = stacked + retreating_steps
                > self.scenario.battle_planning_rules.maneuver_stacking_limit;
            let mut previous = objective.0.as_str();
            let mut rough_going = 0;
            for id in route {
                if hexes[id].terrain == Terrain::Mountain
                    || context
                        .side_features(previous, id)
                        .contains(&HexsideFeature::MajorRiver)
                {
                    rough_going += 1;
                }
                previous = id;
            }
            let rear = attacker_hexes
                .iter()
                .map(|hex| hex_distance(hex, last))
                .min()
                .unwrap_or(0);
            (
                ezoc,
                adjacent_enemy,
                overstacked,
                rough_going,
                Reverse(rear),
                route.join(","),
            )
        };
        // A route is complete when it covers the retreat, or when its first hex
        // holds a friendly unit or retreat-reducing terrain (25.7.3).
        let owed = |route: &Vec<&str>| {
            let first = route[0];
            let stops = net_retreat == 2
                && route.len() == 1
                && (context.friendly_occupied.contains(first)
                    || stops_second_retreat_hex(hexes[first]));
            if stops {
                0
            } else {
                net_retreat.saturating_sub(route.len() as u8)
            }
        };
        let best = routes
            .iter()
            .filter(|route| route.len() <= usize::from(net_retreat))
            .min_by_key(|route| {
                let (ezoc, adjacent, overstacked, rough, rear, ids) = score(route);
                (owed(route) + ezoc, adjacent, overstacked, rough, rear, ids)
            });
        match best {
            Some(route) => RetreatRoute {
                unfulfilled: owed(route),
                ezoc_hexes: score(route).0,
                path: route.iter().map(|id| HexId((*id).to_owned())).collect(),
            },
            None => RetreatRoute {
                path: Vec::new(),
                unfulfilled: net_retreat,
                ezoc_hexes: 0,
            },
        }
    }

    /// Strengths, shifts, and final odds (25.3, 25.5).
    fn battle_odds(
        &self,
        side_id: &SideId,
        hex_id: &HexId,
        attackers: &[&UnitState],
        offensive_support: bool,
    ) -> Result<BattleOdds, RuleError> {
        let hexes = &self.scenario.map.hexes;
        let hex = hexes
            .iter()
            .find(|hex| hex.id == *hex_id)
            .expect("validated hex");
        let defender_side = self
            .scenario
            .sides
            .iter()
            .find(|side| side.id != *side_id)
            .map(|side| side.id.clone())
            .expect("scenario has two sides");
        // Seen from the Defender, "enemy" units and ZOCs are the Attacker's.
        let context = MovementContext::new(self, &defender_side);

        let mut attack_strengths = Vec::new();
        for unit in attackers {
            let mut value = u32::from(current_attack(unit)) * SCALE;
            let mut modifiers = Vec::new();
            if unit.disruption.is_some() {
                value /= 2;
                modifiers.push(StrengthModifier::Disrupted);
            }
            if out_of_combat_supply(unit) {
                value /= 2;
                modifiers.push(StrengthModifier::OutOfCombatSupply);
            }
            if is_armored(unit) && (hex.city.is_some() || hex.terrain == Terrain::Mountain) {
                value /= 2;
                modifiers.push(StrengthModifier::ArmorIntoCityOrMountain);
            }
            if let UnitLocation::Hex { hex_id: from } = &unit.location {
                let features = context.side_features(&from.0, &hex_id.0);
                if features.contains(&HexsideFeature::MajorRiver) {
                    value /= 2;
                    modifiers.push(StrengthModifier::MajorRiver);
                } else if features.contains(&HexsideFeature::MinorRiver) {
                    value = value * 3 / 4;
                    modifiers.push(StrengthModifier::MinorRiver);
                }
            }
            let strength = UnitStrength {
                unit_id: unit.id().clone(),
                printed: current_attack(unit),
                adjusted_64ths: value,
                modifiers,
            };
            attack_strengths.push(self.filter(
                filters::ATTACK_STRENGTH,
                self.strength_input(side_id, hex_id, unit),
                strength,
            )?);
        }

        let engaged = self
            .combat
            .as_ref()
            .map(|combat| combat.engaged_unit_ids.clone())
            .unwrap_or_default();
        let contributing: Vec<&UnitState> = self
            .units
            .values()
            .filter(|unit| {
                unit.definition.side_id != *side_id
                    && in_hex(unit, hex_id)
                    && unit.train_status.is_none()
                    && !engaged.contains(unit.id())
            })
            .collect();
        let maneuver: Vec<&&UnitState> = contributing
            .iter()
            .filter(|unit| !unit.is_headquarters())
            .collect();
        let mut defense_strengths = Vec::new();
        let halve = |unit: &UnitState, value: &mut u32, modifiers: &mut Vec<StrengthModifier>| {
            if unit.disruption.is_some() {
                *value /= 2;
                modifiers.push(StrengthModifier::Disrupted);
            }
            if out_of_combat_supply(unit) {
                *value /= 2;
                modifiers.push(StrengthModifier::OutOfCombatSupply);
            }
        };
        if maneuver.is_empty() {
            // 25.2.7: one HQ defends with its Provisional Defense Strength.
            if let Some(hq) = contributing
                .iter()
                .max_by_key(|unit| (current_defense(unit), Reverse(unit.id().clone())))
            {
                let mut value = u32::from(current_defense(hq)) * SCALE;
                let mut modifiers = vec![StrengthModifier::ProvisionalDefense];
                halve(hq, &mut value, &mut modifiers);
                let strength = UnitStrength {
                    unit_id: hq.id().clone(),
                    printed: current_defense(hq),
                    adjusted_64ths: value,
                    modifiers,
                };
                defense_strengths.push(self.filter(
                    filters::DEFENSE_STRENGTH,
                    self.strength_input(side_id, hex_id, hq),
                    strength,
                )?);
            }
        } else {
            for unit in maneuver {
                let mut value = u32::from(current_defense(unit)) * SCALE;
                let mut modifiers = Vec::new();
                halve(unit, &mut value, &mut modifiers);
                if is_soft_maneuver(unit) && gives_soft_cover(hex) {
                    value *= 2;
                    modifiers.push(StrengthModifier::SoftUnitCover);
                }
                let strength = UnitStrength {
                    unit_id: unit.id().clone(),
                    printed: current_defense(unit),
                    adjusted_64ths: value,
                    modifiers,
                };
                defense_strengths.push(self.filter(
                    filters::DEFENSE_STRENGTH,
                    self.strength_input(side_id, hex_id, unit),
                    strength,
                )?);
            }
        }
        let city_defense = if self.is_enemy_free_city(side_id, hex_id) {
            hex.city.as_ref().map_or(0, |city| city.defense)
        } else {
            0
        };

        let attack_total: u32 = attack_strengths
            .iter()
            .map(|unit| unit.adjusted_64ths)
            .sum();
        let defense_total: u32 = defense_strengths
            .iter()
            .map(|unit| unit.adjusted_64ths)
            .sum::<u32>()
            + u32::from(city_defense) * SCALE;
        // 25.3.1.3, 25.3.2: the Attacker rounds down, the Defender up (never below one).
        let total_attack = (attack_total / SCALE) as u16;
        let total_defense = (defense_total.div_ceil(SCALE) as u16).max(1);
        let basic_column = column_for(total_attack, total_defense);

        let mut shifts = Vec::new();
        let terrain = terrain_shift(hex);
        if terrain != 0 {
            shifts.push(ColumnShift {
                reason: ColumnShiftReason::Terrain,
                shift: terrain,
            });
        }
        if let Some(shift) = envelopment_shift(&context, hex_id) {
            shifts.push(shift);
        }
        if offensive_support {
            shifts.push(ColumnShift {
                reason: ColumnShiftReason::OffensiveSupport,
                shift: 1,
            });
        }
        let surprise = side_id.0 == "warsawPact"
            && self.scenario.battle_planning_rules.air_power.surprise_turn == Some(self.game_turn);
        if surprise {
            shifts.push(ColumnShift {
                reason: ColumnShiftReason::Surprise,
                shift: 1,
            });
        }
        let shifts = self.filter(
            filters::COLUMN_SHIFTS,
            ColumnShiftsInput {
                side_id: side_id.clone(),
                objective: hex_id.clone(),
                attacking_unit_ids: attackers.iter().map(|unit| unit.id().clone()).collect(),
                offensive_support,
            },
            shifts,
        )?;
        // 25.5.1.1-2: at most two columns either way, except a WP attack on the
        // turn of Surprise may shift upward without limit.
        let raw: i8 = shifts.iter().map(|shift| shift.shift).sum();
        let net_shift = if surprise {
            raw.max(-2)
        } else {
            raw.clamp(-2, 2)
        };
        let final_column = (basic_column as i32 + i32::from(net_shift))
            .clamp(0, (ODDS_COLUMNS.len() - 1) as i32) as usize;

        let possible_results = (1..=6)
            .map(|roll| {
                self.combat_result(side_id, hex_id, final_column, roll)
                    .map(|result| result.code)
            })
            .collect::<Result<_, _>>()?;
        Ok(BattleOdds {
            attackers: attack_strengths,
            defenders: defense_strengths,
            city_defense,
            total_attack,
            total_defense,
            basic_column,
            shifts,
            net_shift,
            final_column,
            final_odds: ODDS_COLUMNS[final_column].to_owned(),
            possible_results,
        })
    }

    /// Filter context for one unit's strength in a battle.
    fn strength_input(
        &self,
        side_id: &SideId,
        hex_id: &HexId,
        unit: &UnitState,
    ) -> UnitStrengthInput {
        UnitStrengthInput {
            side_id: side_id.clone(),
            objective: hex_id.clone(),
            unit_id: unit.id().clone(),
        }
    }

    /// Reads the Combat Results Table for a die roll on a column (25.6).
    fn combat_result(
        &self,
        side_id: &SideId,
        hex_id: &HexId,
        final_column: usize,
        die_roll: u8,
    ) -> Result<CombatResult, RuleError> {
        self.filter(
            filters::COMBAT_RESULT,
            CombatResultInput {
                side_id: side_id.clone(),
                objective: hex_id.clone(),
                final_column,
                die_roll,
            },
            parse_result(CRT[usize::from(die_roll) - 1][final_column]),
        )
    }

    // ------------------------------------------------------------------ offensive support

    /// HQs that could give Offensive Support to an attack by these units.
    fn available_support(&self, side_id: &SideId, attackers: &[&UnitState]) -> Vec<UnitId> {
        self.scenario
            .offensive_support_hqs
            .iter()
            .filter(|support| {
                self.validate_support(side_id, &support.hq_id, attackers)
                    .is_ok()
            })
            .map(|support| support.hq_id.clone())
            .collect()
    }

    /// Checks an HQ's ownership, readiness, prior support use, and range to committed Subordinates (25.4).
    fn validate_support(
        &self,
        side_id: &SideId,
        hq_id: &UnitId,
        attackers: &[&UnitState],
    ) -> Result<(), RuleError> {
        // 25.4: a listed HQ of the attacking side, in Supply, not Suppressed, not
        // under a train marker, unused this phase, and within its Support Range
        // of at least one committed Subordinate unit.
        let unavailable = |message: &str| Err(RuleError::new("supportUnavailable", message));
        let Some(support) = self
            .scenario
            .offensive_support_hqs
            .iter()
            .find(|support| support.hq_id == *hq_id)
        else {
            return unavailable("That HQ cannot provide Offensive Support in this scenario");
        };
        let Some(hq) = self.units.get(hq_id) else {
            return unavailable("That HQ is not in play");
        };
        let UnitLocation::Hex { hex_id: hq_hex } = &hq.location else {
            return unavailable("The HQ must be on the map");
        };
        if hq.definition.side_id != *side_id {
            return unavailable("Only a friendly HQ may provide Offensive Support");
        }
        if hq.disruption.is_some()
            || hq.train_status.is_some()
            || hq.supply.headquarters != Some(SupplyStatus::Supplied)
        {
            return unavailable(
                "A Suppressed, entrained, or unsupplied HQ cannot provide Offensive Support",
            );
        }
        if self
            .combat
            .as_ref()
            .is_some_and(|combat| combat.supporting_hq_ids.contains(hq_id))
        {
            return unavailable("Each HQ may support only one battle per Combat Phase");
        }
        let range = support_range(hq);
        let distances = self.support_distances(side_id, hq_hex, range);
        let reaches = attackers.iter().any(|unit| {
            let subordinate = unit
                .definition
                .formation_id
                .as_ref()
                .is_some_and(|formation| support.formations.contains(formation));
            subordinate
                && matches!(&unit.location, UnitLocation::Hex { hex_id } if distances.contains_key(hex_id.0.as_str()))
        });
        if reaches {
            Ok(())
        } else {
            unavailable("No committed Subordinate unit is within the HQ's Support Range")
        }
    }

    /// Searches legal HQ support routes within range while respecting enemy control and impassable terrain (3.4.2, 8.4).
    fn support_distances(
        &self,
        side_id: &SideId,
        from: &HexId,
        range: u16,
    ) -> HashMap<String, u16> {
        // 3.4.2, 8.4: hexes reachable from the HQ within its Support Range,
        // never through enemy units or enemy-controlled cities, EZOC hexes without
        // a friendly unit, All-Sea hexes, or Blocked and All-Sea hexsides (the
        // Danish Ferry excepted). The HQ's hex counts; the unit's does not.
        let context = MovementContext::new(self, side_id);
        los_distances(&context, &from.0, range, true)
    }

    // ------------------------------------------------------------------ eligibility

    /// Classifies a hex as defended or breakthrough-only, returning none for illegal objectives.
    fn objective_kind(&self, side_id: &SideId, hex_id: &HexId) -> Option<ObjectiveKind> {
        let occupied = self
            .units
            .values()
            .any(|unit| unit.definition.side_id != *side_id && in_hex(unit, hex_id));
        if occupied || self.is_enemy_free_city(side_id, hex_id) {
            Some(ObjectiveKind::Defended)
        } else if self.breakthrough_markers.contains(hex_id) {
            Some(ObjectiveKind::BreakthroughOnly)
        } else {
            None
        }
    }

    /// Checks whether an adjacent maneuver unit may attack across the shared hexside in this phase (25.2).
    fn can_attack(
        &self,
        context: &MovementContext,
        side_id: &SideId,
        unit: &UnitState,
        hex_id: &HexId,
    ) -> bool {
        // 25.2: an adjacent Maneuver unit that has not attacked, is not under a
        // train marker (13.4.2), and does not face a Blocked, All-Sea, or Danish
        // Ferry hexside (25.2.4).
        let UnitLocation::Hex { hex_id: from } = &unit.location else {
            return false;
        };
        unit.definition.side_id == *side_id
            && !unit.is_headquarters()
            && unit.train_status.is_none()
            // 12.6: units under a Reserve/OMG Marker never attack.
            && !self.is_reserve_unit(unit.id())
            && !self
                .combat
                .as_ref()
                .is_some_and(|combat| combat.attacked_unit_ids.contains(unit.id()))
            && context.neighbors(&from.0).contains(&hex_id.0.as_str())
            && !context.prohibited_hexside(&from.0, &hex_id.0)
            && !context
                .side_features(&from.0, &hex_id.0)
                .contains(&HexsideFeature::DanishFerry)
    }

    /// Checks the objective and distinct eligible attackers, requiring attack strength for defended hexes.
    fn validate_attack(
        &self,
        hex_id: &HexId,
        unit_ids: &[UnitId],
    ) -> Result<Vec<&UnitState>, RuleError> {
        let options = self.combat_options()?;
        let objective = options
            .objectives
            .iter()
            .find(|objective| objective.hex_id == *hex_id)
            .ok_or_else(|| {
                RuleError::new(
                    "invalidObjective",
                    "That hex cannot be attacked now (no enemy, not marked, already attacked, or no adjacent attackers)",
                )
            })?;
        if unit_ids.is_empty() {
            return Err(RuleError::new(
                "noAttackers",
                "Commit at least one adjacent Maneuver unit",
            ));
        }
        let mut units = Vec::new();
        for id in unit_ids {
            if !objective.eligible_unit_ids.contains(id) {
                return Err(RuleError::new(
                    "unitCannotAttack",
                    format!("{} cannot attack this hex", id.0),
                ));
            }
            if units.iter().any(|unit: &&UnitState| unit.id() == id) {
                return Err(RuleError::new(
                    "unitCannotAttack",
                    "A unit may be committed only once",
                ));
            }
            units.push(&self.units[id]);
        }
        // 25.2.2: zero-strength units may join but never attack alone.
        if !objective.breakthrough_only && units.iter().all(|unit| current_attack(unit) == 0) {
            return Err(RuleError::new(
                "noAttackStrength",
                "Units with an Attack Strength of zero may not attack alone",
            ));
        }
        Ok(units)
    }

    /// Returns the acting side after verifying that a single-side Combat Phase is active.
    fn combat_side(&self) -> Result<SideId, RuleError> {
        let Some(step) = self.current_step() else {
            return Err(crate::error::game_complete());
        };
        if step.phase_id.0 != "combat" {
            return Err(RuleError::new(
                "wrongPhase",
                "This command is available only during the Combat Phase",
            ));
        }
        match &step.actor {
            PhaseActor::Side { side_id } => Ok(side_id.clone()),
            PhaseActor::All => Err(RuleError::new(
                "wrongActor",
                "The Combat Phase must have one acting side",
            )),
        }
    }

    /// Borrows the current combat state or reports that no Combat Phase is in progress.
    fn combat_state(&self) -> Result<&CombatState, RuleError> {
        self.combat
            .as_ref()
            .ok_or_else(|| RuleError::new("combatUnavailable", "No Combat Phase is in progress"))
    }
}
