//! Named air-counter planning, fighter combat, interception, and automatic strikes.

use std::collections::{BTreeMap, BTreeSet};

use crate::airspace::hex_distance;
use crate::error::RuleError;
use crate::event::GameEvent;
use crate::model::{
    AirBaseId, AirCombatAttack, AirCombatPairing, AirCombatResult, AirGroundTarget,
    AirMissionAssignment, AirMissionOptions, AirOperationsReport, AirPlan, AirPlanningOptions,
    AirReadiness, AirSortie, AirSortieStatus, AirUnitId, AirUnitKind, AirUnitPlanningOption, HexId,
    PhaseActor, SideId, StrikeResult, UnitId, UnitLocation,
};
use crate::rules::Rules;
use crate::strikes::strike_table;

const MAX_AIR_COMBAT_ROUNDS: u8 = 20;

#[derive(Clone)]
struct FighterNode {
    id: AirUnitId,
    side_id: SideId,
    center: HexId,
    radius: u8,
}

#[derive(Clone)]
struct BomberNode {
    id: AirUnitId,
    side_id: SideId,
    target: HexId,
}

/// Reads a modified d20 roll on the 1985 Air Combat Table.
pub fn air_combat_table(column: i8, modified_roll: i8) -> AirCombatResult {
    let roll = modified_roll.clamp(1, 20);
    match column.clamp(-4, 4) {
        -4 => match roll {
            1..=15 => AirCombatResult::NoEffect,
            16..=18 => AirCombatResult::Abort,
            _ => AirCombatResult::DamagedAbort,
        },
        -3 => match roll {
            1..=13 => AirCombatResult::NoEffect,
            14..=16 => AirCombatResult::Abort,
            17..=19 => AirCombatResult::DamagedAbort,
            _ => AirCombatResult::DestroyedAbort,
        },
        -2 => match roll {
            1..=11 => AirCombatResult::NoEffect,
            12..=14 => AirCombatResult::Abort,
            15..=17 => AirCombatResult::DamagedAbort,
            _ => AirCombatResult::DestroyedAbort,
        },
        -1 => match roll {
            1..=10 => AirCombatResult::NoEffect,
            11..=13 => AirCombatResult::Abort,
            14..=16 => AirCombatResult::DamagedAbort,
            17..=19 => AirCombatResult::DestroyedAbort,
            _ => AirCombatResult::DestroyedAndDamagedAbort,
        },
        0 => match roll {
            1..=9 => AirCombatResult::NoEffect,
            10..=12 => AirCombatResult::Abort,
            13..=15 => AirCombatResult::DamagedAbort,
            16..=18 => AirCombatResult::DestroyedAbort,
            _ => AirCombatResult::DestroyedAndDamagedAbort,
        },
        1 => match roll {
            1..=8 => AirCombatResult::NoEffect,
            9..=11 => AirCombatResult::Abort,
            12..=14 => AirCombatResult::DamagedAbort,
            15..=17 => AirCombatResult::DestroyedAbort,
            _ => AirCombatResult::DestroyedAndDamagedAbort,
        },
        2 => match roll {
            1..=7 => AirCombatResult::NoEffect,
            8..=10 => AirCombatResult::Abort,
            11..=13 => AirCombatResult::DamagedAbort,
            14..=16 => AirCombatResult::DestroyedAbort,
            _ => AirCombatResult::DestroyedAndDamagedAbort,
        },
        3 => match roll {
            1..=5 => AirCombatResult::NoEffect,
            6..=8 => AirCombatResult::Abort,
            9..=11 => AirCombatResult::DamagedAbort,
            12..=14 => AirCombatResult::DestroyedAbort,
            _ => AirCombatResult::DestroyedAndDamagedAbort,
        },
        _ => match roll {
            1..=3 => AirCombatResult::NoEffect,
            4..=6 => AirCombatResult::Abort,
            7..=9 => AirCombatResult::DamagedAbort,
            10..=12 => AirCombatResult::DestroyedAbort,
            _ => AirCombatResult::DestroyedAndDamagedAbort,
        },
    }
}

impl Rules {
    /// Makes every surviving aircraft ready and clears the previous operation report.
    pub(crate) fn ready_air_units(&mut self, events: &mut Vec<GameEvent>) {
        for unit in self.air_units.values_mut() {
            unit.readiness = AirReadiness::Ready;
        }
        self.air_plans.clear();
        self.air_operations_report = None;
        events.push(GameEvent::AirUnitsReadied {
            air_units: self.air_units.values().cloned().collect(),
        });
    }

    /// Creates or replaces the acting side's empty current-turn air plan.
    pub(crate) fn start_air_plan(&mut self) {
        let Some(side_id) = self.acting_side() else {
            return;
        };
        self.air_plans.retain(|plan| plan.side_id != side_id);
        self.air_plans.push(AirPlan::new(self.game_turn, side_id));
    }

    fn air_plan(&self, side_id: &SideId) -> Option<&AirPlan> {
        self.air_plans
            .iter()
            .find(|plan| plan.side_id == *side_id && plan.game_turn == self.game_turn)
    }

    fn air_plan_mut(&mut self, side_id: &SideId) -> Option<&mut AirPlan> {
        self.air_plans
            .iter_mut()
            .find(|plan| plan.side_id == *side_id && plan.game_turn == self.game_turn)
    }

    fn air_unit_unavailable(&self, unit_id: &AirUnitId, side_id: &SideId) -> Option<RuleError> {
        let Some(unit) = self.air_units.get(unit_id) else {
            return Some(RuleError::new(
                "unknownAirUnit",
                format!("Unknown air unit: {}", unit_id.0),
            ));
        };
        if unit.definition.side_id != *side_id {
            return Some(RuleError::new(
                "airUnitNotControlled",
                "The acting side does not control this air unit",
            ));
        }
        if unit.readiness != AirReadiness::Ready {
            return Some(RuleError::new(
                "airUnitUnavailable",
                "The air unit is not ready",
            ));
        }
        if self.air_plan(side_id).is_some_and(|plan| {
            plan.sorties
                .iter()
                .any(|sortie| sortie.air_unit_id == *unit_id)
        }) {
            return Some(RuleError::new(
                "airUnitAlreadyAssigned",
                "The air unit already has a mission",
            ));
        }
        let Some(base) = self.air_bases.get(&unit.definition.base_id) else {
            return Some(RuleError::new(
                "unknownAirBase",
                "The air unit's base does not exist",
            ));
        };
        let used = self.air_plan(side_id).map_or(0, |plan| {
            plan.sorties
                .iter()
                .filter(|sortie| {
                    self.air_units
                        .get(&sortie.air_unit_id)
                        .is_some_and(|other| other.definition.base_id == base.definition.id)
                })
                .count() as u8
        });
        if used >= base.effective_capacity(self.game_turn) {
            return Some(RuleError::new(
                "airBaseCapacityExceeded",
                "The airbase has no sortie capacity remaining",
            ));
        }
        None
    }

    /// Read-only availability and base-capacity preview for the planning side.
    pub fn air_planning_options(&self) -> Result<AirPlanningOptions, RuleError> {
        let side_id = self.battle_planning_side()?;
        let units = self
            .air_units
            .values()
            .filter(|unit| unit.definition.side_id == side_id)
            .map(|unit| AirUnitPlanningOption {
                air_unit_id: unit.definition.id.clone(),
                unavailable: self.air_unit_unavailable(&unit.definition.id, &side_id),
            })
            .collect();
        let bases = self
            .air_bases
            .values()
            .filter(|base| base.definition.side_id == side_id)
            .cloned()
            .collect();
        Ok(AirPlanningOptions {
            side_id,
            units,
            bases,
        })
    }

    /// Legal target families for one air counter.
    pub fn air_mission_options(
        &self,
        air_unit_id: &AirUnitId,
    ) -> Result<AirMissionOptions, RuleError> {
        let side_id = self.battle_planning_side()?;
        if let Some(error) = self.air_unit_unavailable(air_unit_id, &side_id) {
            return Err(error);
        }
        let unit = &self.air_units[air_unit_id];
        let center_hexes = if matches!(
            unit.definition.kind,
            AirUnitKind::Fighter | AirUnitKind::Aew
        ) {
            self.scenario
                .map
                .hexes
                .iter()
                .map(|hex| hex.id.clone())
                .collect()
        } else {
            Vec::new()
        };
        let mut by_hex: BTreeMap<HexId, Vec<UnitId>> = BTreeMap::new();
        if unit.definition.kind == AirUnitKind::FighterBomber {
            for target in self
                .units
                .values()
                .filter(|target| target.definition.side_id != side_id)
            {
                if let UnitLocation::Hex { hex_id } = &target.location {
                    by_hex
                        .entry(hex_id.clone())
                        .or_default()
                        .push(target.id().clone());
                }
            }
        }
        let ground_targets = by_hex
            .into_iter()
            .map(|(hex_id, unit_ids)| AirGroundTarget { hex_id, unit_ids })
            .collect();
        let air_base_ids = if unit.definition.kind == AirUnitKind::FighterBomber {
            self.air_bases
                .values()
                .filter(|base| base.definition.side_id != side_id)
                .map(|base| base.definition.id.clone())
                .collect()
        } else {
            Vec::new()
        };
        Ok(AirMissionOptions {
            air_unit_id: air_unit_id.clone(),
            kind: unit.definition.kind,
            center_hexes,
            ground_targets,
            air_base_ids,
        })
    }

    /// Validates and records one sortie during Battle Planning.
    pub(super) fn plan_air_sortie(
        &mut self,
        air_unit_id: AirUnitId,
        mission: AirMissionAssignment,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.battle_planning_side()?;
        if let Some(error) = self.air_unit_unavailable(&air_unit_id, &side_id) {
            return Err(error);
        }
        let unit = self.air_units[&air_unit_id].clone();
        self.validate_air_mission(&side_id, &unit.definition.kind, &mission)?;
        let plan = self
            .air_plan_mut(&side_id)
            .ok_or_else(|| RuleError::new("airPlanUnavailable", "No current air plan exists"))?;
        let sortie = AirSortie {
            id: plan.next_sortie_id,
            air_unit_id,
            mission,
            status: AirSortieStatus::Planned,
        };
        plan.next_sortie_id += 1;
        plan.sorties.push(sortie.clone());
        Ok(vec![GameEvent::AirSortiePlanned { side_id, sortie }])
    }

    fn validate_air_mission(
        &self,
        side_id: &SideId,
        kind: &AirUnitKind,
        mission: &AirMissionAssignment,
    ) -> Result<(), RuleError> {
        match (kind, mission) {
            (AirUnitKind::Fighter, AirMissionAssignment::AirSuperiority { center_hex_id })
            | (AirUnitKind::Aew, AirMissionAssignment::EarlyWarning { center_hex_id }) => {
                self.map_hex(center_hex_id)?;
            }
            (
                AirUnitKind::FighterBomber,
                AirMissionAssignment::GroundStrike { hex_id, unit_ids },
            ) => {
                self.map_hex(hex_id)?;
                if unit_ids.is_empty() {
                    return Err(RuleError::new(
                        "emptyStrike",
                        "A ground strike must name a target",
                    ));
                }
                if unit_ids.iter().collect::<BTreeSet<_>>().len() != unit_ids.len() {
                    return Err(RuleError::new(
                        "duplicateStrikeTarget",
                        "A ground strike cannot name the same target twice",
                    ));
                }
                let mut steps = 0_u16;
                for id in unit_ids {
                    let target = self.units.get(id).ok_or_else(|| {
                        RuleError::new("unknownUnit", format!("Unknown target: {}", id.0))
                    })?;
                    if target.definition.side_id == *side_id
                        || !matches!(&target.location, UnitLocation::Hex { hex_id: at } if at == hex_id)
                    {
                        return Err(RuleError::new(
                            "invalidStrikeTarget",
                            "Every target must be an enemy unit in the target hex",
                        ));
                    }
                    steps += target.step_count();
                }
                if steps > 2 {
                    return Err(RuleError::new(
                        "strikeStepLimit",
                        "One fighter-bomber may target at most two enemy steps",
                    ));
                }
                if unit_ids.len() > 1 && unit_ids.iter().any(|id| self.units[id].is_headquarters())
                {
                    return Err(RuleError::new(
                        "headquartersTarget",
                        "A headquarters must be struck alone",
                    ));
                }
                if self
                    .air_plans
                    .iter()
                    .flat_map(|plan| &plan.sorties)
                    .any(|sortie| match &sortie.mission {
                        AirMissionAssignment::GroundStrike {
                            unit_ids: prior, ..
                        } => prior.iter().any(|id| unit_ids.contains(id)),
                        _ => false,
                    })
                {
                    return Err(RuleError::new(
                        "unitAlreadyTargeted",
                        "A unit may be named by only one air strike per turn",
                    ));
                }
            }
            (AirUnitKind::FighterBomber, AirMissionAssignment::AirBaseStrike { air_base_id }) => {
                let base = self.air_bases.get(air_base_id).ok_or_else(|| {
                    RuleError::new(
                        "unknownAirBase",
                        format!("Unknown airbase: {}", air_base_id.0),
                    )
                })?;
                if base.definition.side_id == *side_id {
                    return Err(RuleError::new(
                        "friendlyAirBase",
                        "A fighter-bomber may strike only an enemy airbase",
                    ));
                }
            }
            _ => {
                return Err(RuleError::new(
                    "invalidAirMission",
                    "This air-unit type cannot perform that mission",
                ))
            }
        }
        Ok(())
    }

    /// Withdraws one unresolved sortie from the acting side's air plan.
    pub(super) fn cancel_air_sortie(
        &mut self,
        sortie_id: u32,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.battle_planning_side()?;
        let plan = self
            .air_plan_mut(&side_id)
            .ok_or_else(|| RuleError::new("airPlanUnavailable", "No current air plan exists"))?;
        let index = plan
            .sorties
            .iter()
            .position(|sortie| sortie.id == sortie_id)
            .ok_or_else(|| {
                RuleError::new("unknownSortie", "No such sortie exists in the active plan")
            })?;
        plan.sorties.remove(index);
        Ok(vec![GameEvent::AirSortieCancelled { side_id, sortie_id }])
    }

    /// Resolves all fighter-vs-fighter combat and then fighter interception.
    pub(crate) fn resolve_joint_air_operations(&mut self, events: &mut Vec<GameEvent>) {
        let mut report = AirOperationsReport {
            game_turn: self.game_turn,
            fighter_combat: Vec::new(),
            interceptions: Vec::new(),
            round_limit_reached: false,
        };
        for round in 1..=MAX_AIR_COMBAT_ROUNDS {
            let fighters = self.active_fighters();
            let pairs = self.fighter_matching(&fighters);
            if pairs.is_empty() {
                break;
            }
            for (a, b) in pairs {
                let pairing = self.resolve_air_pair(&a.id, &a.center, &b.id, &b.center, round);
                events.push(GameEvent::AirCombatRoundResolved {
                    pairing: pairing.clone(),
                });
                self.apply_pairing(&pairing, events);
                report.fighter_combat.push(pairing);
            }
            if round == MAX_AIR_COMBAT_ROUNDS {
                report.round_limit_reached = true;
            }
        }

        let fighters = self.active_fighters();
        let bombers = self.active_bombers();
        for (fighter, bomber) in self.interception_matching(&fighters, &bombers) {
            let pairing =
                self.resolve_air_pair(&fighter.id, &fighter.center, &bomber.id, &bomber.target, 1);
            events.push(GameEvent::AirInterceptionResolved {
                pairing: pairing.clone(),
            });
            self.apply_pairing(&pairing, events);
            report.interceptions.push(pairing);
        }

        let ids: Vec<_> = self.air_units.keys().cloned().collect();
        for id in ids {
            if self.current_sortie(&id).is_none() {
                continue;
            }
            let Some(unit) = self.air_units.get_mut(&id) else {
                continue;
            };
            if unit.readiness == AirReadiness::Aborted {
                self.set_sortie_status(&id, AirSortieStatus::Aborted);
            } else if unit.definition.kind == AirUnitKind::FighterBomber {
                self.set_sortie_status(&id, AirSortieStatus::Cleared);
            } else {
                unit.readiness = AirReadiness::Flown;
                self.set_sortie_status(&id, AirSortieStatus::Completed);
            }
        }
        self.air_operations_report = Some(report);
    }

    fn active_fighters(&self) -> Vec<FighterNode> {
        let mut result: Vec<_> = self
            .air_plans
            .iter()
            .flat_map(|plan| &plan.sorties)
            .filter_map(|sortie| {
                let AirMissionAssignment::AirSuperiority { center_hex_id } = &sortie.mission else {
                    return None;
                };
                let unit = self.air_units.get(&sortie.air_unit_id)?;
                (unit.readiness == AirReadiness::Ready).then(|| FighterNode {
                    id: unit.definition.id.clone(),
                    side_id: unit.definition.side_id.clone(),
                    center: center_hex_id.clone(),
                    radius: unit.current_step().combat_radius,
                })
            })
            .collect();
        result.sort_by(|a, b| a.id.cmp(&b.id));
        result
    }

    fn active_bombers(&self) -> Vec<BomberNode> {
        let mut result: Vec<_> = self
            .air_plans
            .iter()
            .flat_map(|plan| &plan.sorties)
            .filter_map(|sortie| {
                let unit = self.air_units.get(&sortie.air_unit_id)?;
                if unit.definition.kind != AirUnitKind::FighterBomber
                    || unit.readiness != AirReadiness::Ready
                {
                    return None;
                }
                Some(BomberNode {
                    id: unit.definition.id.clone(),
                    side_id: unit.definition.side_id.clone(),
                    target: sortie.mission.target_hex(&self.air_bases)?.clone(),
                })
            })
            .collect();
        result.sort_by(|a, b| a.id.cmp(&b.id));
        result
    }

    fn fighter_matching(&self, nodes: &[FighterNode]) -> Vec<(FighterNode, FighterNode)> {
        let left: Vec<_> = nodes
            .iter()
            .filter(|node| node.side_id.0 == "warsawPact")
            .cloned()
            .collect();
        let right: Vec<_> = nodes
            .iter()
            .filter(|node| node.side_id.0 != "warsawPact")
            .cloned()
            .collect();
        let adjacency: Vec<Vec<usize>> = left
            .iter()
            .map(|a| {
                let mut edges: Vec<_> = right
                    .iter()
                    .enumerate()
                    .filter_map(|(index, b)| {
                        let distance = self.distance(&a.center, &b.center)?;
                        (distance <= i32::from(a.radius + b.radius)).then_some((
                            distance,
                            b.id.clone(),
                            index,
                        ))
                    })
                    .collect();
                edges.sort();
                edges.into_iter().map(|(_, _, index)| index).collect()
            })
            .collect();
        maximum_matching(&left, &right, &adjacency)
    }

    fn interception_matching(
        &self,
        fighters: &[FighterNode],
        bombers: &[BomberNode],
    ) -> Vec<(FighterNode, BomberNode)> {
        let adjacency: Vec<Vec<usize>> = fighters
            .iter()
            .map(|fighter| {
                let mut edges: Vec<_> = bombers
                    .iter()
                    .enumerate()
                    .filter(|(_, bomber)| bomber.side_id != fighter.side_id)
                    .filter_map(|(index, bomber)| {
                        let distance = self.distance(&fighter.center, &bomber.target)?;
                        (distance <= i32::from(fighter.radius)).then_some((
                            distance,
                            bomber.id.clone(),
                            index,
                        ))
                    })
                    .collect();
                edges.sort();
                edges.into_iter().map(|(_, _, index)| index).collect()
            })
            .collect();
        maximum_matching(fighters, bombers, &adjacency)
    }

    fn distance(&self, a: &HexId, b: &HexId) -> Option<i32> {
        let first = self.scenario.map.hexes.iter().find(|hex| hex.id == *a)?;
        let second = self.scenario.map.hexes.iter().find(|hex| hex.id == *b)?;
        Some(hex_distance(first, second))
    }

    fn resolve_air_pair(
        &mut self,
        first: &AirUnitId,
        first_at: &HexId,
        second: &AirUnitId,
        second_at: &HexId,
        round: u8,
    ) -> AirCombatPairing {
        let first_attack = self.roll_air_attack(first, second, first_at);
        let second_attack = self.roll_air_attack(second, first, second_at);
        AirCombatPairing {
            round,
            first: first_attack,
            second: second_attack,
        }
    }

    fn roll_air_attack(
        &mut self,
        attacker: &AirUnitId,
        defender: &AirUnitId,
        attacker_at: &HexId,
    ) -> AirCombatAttack {
        let attacker_state = self.air_units[attacker].clone();
        let defender_state = self.air_units[defender].clone();
        let column = (attacker_state.current_step().air_combat
            - defender_state.current_step().evasion)
            .clamp(-4, 4);
        let modifier = self.aew_modifier(&attacker_state.definition.side_id, attacker_at);
        let die_roll = self.dice.d20();
        let modified_roll = die_roll as i8 + modifier;
        AirCombatAttack {
            attacker_id: attacker.clone(),
            defender_id: defender.clone(),
            column,
            die_roll,
            modifier,
            modified_roll,
            result: air_combat_table(column, modified_roll),
        }
    }

    fn aew_modifier(&self, side_id: &SideId, at: &HexId) -> i8 {
        self.air_plans
            .iter()
            .filter(|plan| plan.side_id == *side_id)
            .flat_map(|plan| &plan.sorties)
            .filter_map(|sortie| {
                let AirMissionAssignment::EarlyWarning { center_hex_id } = &sortie.mission else {
                    return None;
                };
                let unit = self.air_units.get(&sortie.air_unit_id)?;
                if unit.readiness != AirReadiness::Ready {
                    return None;
                }
                let distance = self.distance(center_hex_id, at)?;
                (distance <= i32::from(unit.current_step().aew_radius))
                    .then_some(unit.current_step().aew_modifier)
            })
            .max()
            .unwrap_or(0)
    }

    fn apply_pairing(&mut self, pairing: &AirCombatPairing, events: &mut Vec<GameEvent>) {
        self.apply_air_result(&pairing.first.defender_id, pairing.first.result, events);
        self.apply_air_result(&pairing.second.defender_id, pairing.second.result, events);
    }

    fn apply_air_result(
        &mut self,
        unit_id: &AirUnitId,
        result: AirCombatResult,
        events: &mut Vec<GameEvent>,
    ) {
        let aborts_strike = result.aborts()
            && self.current_sortie(unit_id).is_some_and(|sortie| {
                matches!(
                    sortie.mission,
                    AirMissionAssignment::GroundStrike { .. }
                        | AirMissionAssignment::AirBaseStrike { .. }
                )
            });
        let losses = result.step_losses();
        if losses > 0 {
            let remaining = self
                .air_units
                .get(unit_id)
                .map_or(0, |unit| unit.remaining_steps());
            if losses >= remaining {
                self.air_units.remove(unit_id);
                self.eliminated_air_units.push(unit_id.clone());
                events.push(GameEvent::AirUnitEliminated {
                    air_unit_id: unit_id.clone(),
                });
                self.set_sortie_status(unit_id, AirSortieStatus::Aborted);
                if aborts_strike {
                    events.push(GameEvent::AirStrikeAborted {
                        air_unit_id: unit_id.clone(),
                    });
                }
                return;
            }
            let unit = self
                .air_units
                .get_mut(unit_id)
                .expect("surviving air unit exists");
            unit.strength_step_index += usize::from(losses);
            events.push(GameEvent::AirUnitStepLost {
                air_unit_id: unit_id.clone(),
                strength_step_index: unit.strength_step_index,
            });
        }
        if result.aborts() {
            if let Some(unit) = self.air_units.get_mut(unit_id) {
                unit.readiness = AirReadiness::Aborted;
                events.push(GameEvent::AirUnitAborted {
                    air_unit_id: unit_id.clone(),
                });
            }
            self.set_sortie_status(unit_id, AirSortieStatus::Aborted);
            if aborts_strike {
                events.push(GameEvent::AirStrikeAborted {
                    air_unit_id: unit_id.clone(),
                });
            }
        }
    }

    fn current_sortie(&self, unit_id: &AirUnitId) -> Option<&AirSortie> {
        self.air_plans
            .iter()
            .filter(|plan| plan.game_turn == self.game_turn)
            .flat_map(|plan| &plan.sorties)
            .find(|sortie| sortie.air_unit_id == *unit_id)
    }

    fn set_sortie_status(&mut self, unit_id: &AirUnitId, status: AirSortieStatus) {
        for sortie in self.air_plans.iter_mut().flat_map(|plan| &mut plan.sorties) {
            if sortie.air_unit_id == *unit_id {
                sortie.status = status;
            }
        }
    }

    /// Resolves every cleared fighter-bomber mission belonging to the acting side.
    pub(crate) fn resolve_air_strike_phase(&mut self, events: &mut Vec<GameEvent>) {
        let Some(PhaseActor::Side { side_id }) = self.current_step().map(|step| step.actor.clone())
        else {
            return;
        };
        let missions: Vec<_> = self
            .air_plan(&side_id)
            .into_iter()
            .flat_map(|plan| &plan.sorties)
            .filter(|sortie| sortie.status == AirSortieStatus::Cleared)
            .cloned()
            .collect();
        for sortie in missions {
            let Some(unit) = self.air_units.get(&sortie.air_unit_id).cloned() else {
                continue;
            };
            let strike_modifier = unit.current_step().strike_modifier;
            match &sortie.mission {
                AirMissionAssignment::GroundStrike { hex_id, unit_ids } => {
                    let surviving: Vec<_> = unit_ids
                        .iter()
                        .filter(|id| self.units.contains_key(*id))
                        .cloned()
                        .collect();
                    if surviving.is_empty() {
                        self.set_sortie_status(&sortie.air_unit_id, AirSortieStatus::TargetGone);
                        continue;
                    }
                    let mut consequences = Vec::new();
                    if let Ok(resolution) = self.resolve_strike(
                        &side_id,
                        hex_id,
                        &surviving,
                        strike_modifier,
                        &mut consequences,
                    ) {
                        events.push(GameEvent::AirStrikeResolved {
                            side_id: side_id.clone(),
                            air_unit_id: Some(sortie.air_unit_id.clone()),
                            mission_id: sortie.id,
                            hex_id: hex_id.clone(),
                            unit_ids: surviving,
                            resolution,
                        });
                        events.extend(consequences);
                    }
                }
                AirMissionAssignment::AirBaseStrike { air_base_id } => {
                    self.resolve_air_base_strike(
                        &side_id,
                        &sortie.air_unit_id,
                        sortie.id,
                        air_base_id,
                        strike_modifier,
                        events,
                    );
                }
                _ => {}
            }
            if let Some(air_unit) = self.air_units.get_mut(&sortie.air_unit_id) {
                air_unit.readiness = AirReadiness::Flown;
            }
            self.set_sortie_status(&sortie.air_unit_id, AirSortieStatus::Completed);
        }
    }

    fn resolve_air_base_strike(
        &mut self,
        side_id: &SideId,
        air_unit_id: &AirUnitId,
        mission_id: u32,
        air_base_id: &AirBaseId,
        aircraft_modifier: i8,
        events: &mut Vec<GameEvent>,
    ) {
        let Some(base) = self.air_bases.get(air_base_id).cloned() else {
            return;
        };
        let die_roll = self.dice.d6();
        let modifier = base.definition.strike_modifier + aircraft_modifier;
        let modified_roll = die_roll as i8 + modifier;
        let result = strike_table(modified_roll);
        events.push(GameEvent::AirStrikeResolved {
            side_id: side_id.clone(),
            air_unit_id: Some(air_unit_id.clone()),
            mission_id,
            hex_id: base.definition.anchor_hex_id,
            unit_ids: Vec::new(),
            resolution: crate::model::StrikeResolution {
                die_roll,
                modifier,
                modified_roll,
                result,
            },
        });
        if matches!(result, StrikeResult::Disrupted | StrikeResult::StepLoss) {
            let through_turn = self.game_turn.saturating_add(1);
            if let Some(target) = self.air_bases.get_mut(air_base_id) {
                target.suppressed_through_turn = Some(through_turn);
            }
            events.push(GameEvent::AirBaseSuppressed {
                air_base_id: air_base_id.clone(),
                through_turn,
            });
        }
        if result == StrikeResult::StepLoss {
            let target = self
                .air_bases
                .get_mut(air_base_id)
                .expect("validated airbase exists");
            target.damage = target.damage.saturating_add(1).min(2);
            events.push(GameEvent::AirBaseDamaged {
                air_base_id: air_base_id.clone(),
                damage: target.damage,
            });
        }
    }
}

fn maximum_matching<L: Clone, R: Clone>(
    left: &[L],
    right: &[R],
    adjacency: &[Vec<usize>],
) -> Vec<(L, R)> {
    fn augment(
        index: usize,
        adjacency: &[Vec<usize>],
        seen: &mut BTreeSet<usize>,
        matched: &mut [Option<usize>],
    ) -> bool {
        for &candidate in &adjacency[index] {
            if !seen.insert(candidate) {
                continue;
            }
            if matched[candidate].is_none()
                || augment(matched[candidate].unwrap(), adjacency, seen, matched)
            {
                matched[candidate] = Some(index);
                return true;
            }
        }
        false
    }

    let mut matched = vec![None; right.len()];
    for index in 0..left.len() {
        augment(index, adjacency, &mut BTreeSet::new(), &mut matched);
    }
    let mut pairs: Vec<_> = matched
        .into_iter()
        .enumerate()
        .filter_map(|(right_index, left_index)| {
            left_index.map(|left_index| (left_index, right_index))
        })
        .collect();
    pairs.sort();
    pairs
        .into_iter()
        .map(|(left_index, right_index)| (left[left_index].clone(), right[right_index].clone()))
        .collect()
}
