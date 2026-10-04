use crate::error::RuleError;
use crate::event::GameEvent;
use crate::model::{
    Airspace, BattlePlan, HexId, MovementMode, PhaseActor, PlannedMovement, SideId, SupplyStatus,
    TrainStatus, UnitId, UnitLocation, UnitState,
};
use crate::reserve::MovementPhase;
use crate::rules::Rules;

impl Rules {
    /// Selects or deselects an eligible resupply unit while enforcing the plan's operation limit.
    pub(super) fn set_resupply_target(
        &mut self,
        unit_id: UnitId,
        selected: bool,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.battle_planning_side()?;
        if selected {
            let unit = self.owned_unit(&unit_id, &side_id)?;
            if unit.is_headquarters() {
                return Err(RuleError::new(
                    "invalidResupplyTarget",
                    "Resupply operations may target combat units, not headquarters",
                ));
            }
            if !matches!(unit.location, UnitLocation::Hex { .. }) {
                return Err(RuleError::new(
                    "invalidResupplyTarget",
                    "The resupply target must be on the map",
                ));
            }

            let limit = usize::from(
                self.scenario
                    .battle_planning_rules
                    .resupply_operations_per_turn,
            );
            if limit == 0 {
                return Err(RuleError::new(
                    "resupplyUnavailable",
                    "This scenario does not provide a resupply operation",
                ));
            }

            let targets = &mut self.active_plan_mut()?.resupply_target_unit_ids;
            if !targets.contains(&unit_id) {
                if targets.len() >= limit {
                    return Err(RuleError::new(
                        "resupplyLimitReached",
                        format!("This plan allows at most {limit} resupply operations"),
                    ));
                }
                targets.push(unit_id.clone());
            }
        } else {
            self.active_plan_mut()?
                .resupply_target_unit_ids
                .retain(|target| target != &unit_id);
        }

        Ok(vec![GameEvent::ResupplyTargetSet {
            side_id,
            unit_id,
            selected,
        }])
    }

    /// Selects or deselects a currently eligible enemy objective in the active battle plan.
    pub(super) fn set_attack_target(
        &mut self,
        hex_id: HexId,
        selected: bool,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.battle_planning_side()?;
        if !self.scenario.map.hexes.iter().any(|hex| hex.id == hex_id) {
            return Err(RuleError::new(
                "unknownHex",
                format!("Unknown attack target hex: {}", hex_id.0),
            ));
        }
        if selected && !self.is_attack_target_eligible(&side_id, &hex_id) {
            return Err(RuleError::new(
                "invalidAttackTarget",
                "An attack objective must contain an enemy unit or be an enemy Free City",
            ));
        }

        let targets = &mut self.active_plan_mut()?.attack_targets;
        if selected && !targets.contains(&hex_id) {
            targets.push(hex_id.clone());
            targets.sort_by(|a, b| a.0.cmp(&b.0));
        } else if !selected {
            targets.retain(|target| target != &hex_id);
        }
        Ok(vec![GameEvent::AttackTargetSet {
            side_id,
            hex_id,
            selected,
        }])
    }

    /// Lists hexes the planning side may currently mark as attack objectives.
    ///
    /// Eligible hexes contain enemy units or are enemy Free Cities. This is a
    /// read-only preview; `SetAttackTarget` checks eligibility again when submitted.
    ///
    /// # Returns
    ///
    /// Eligible [`HexId`] values in scenario map order.
    ///
    /// # Errors
    ///
    /// Returns a [`RuleError`] if the game has ended or the current step is not
    /// a Battle Planning Phase with one acting side.
    pub fn attack_target_options(&self) -> Result<Vec<HexId>, RuleError> {
        let side_id = self.battle_planning_side()?;
        Ok(self
            .scenario
            .map
            .hexes
            .iter()
            .filter(|hex| self.is_attack_target_eligible(&side_id, &hex.id))
            .map(|hex| hex.id.clone())
            .collect())
    }

    /// Checks whether a planning objective contains an enemy unit or is an enemy Free City (25.1.1).
    fn is_attack_target_eligible(&self, side_id: &SideId, hex_id: &HexId) -> bool {
        // 25.1.1: an Objective hex contains an enemy Ground unit or is an enemy
        // Free City (which need not be occupied).  Breakthrough Markers do not exist yet.
        self.is_enemy_free_city(side_id, hex_id)
            || self.units.values().any(|unit| {
                unit.definition.side_id != *side_id
                    && matches!(&unit.location, UnitLocation::Hex { hex_id: occupied } if occupied == hex_id)
            })
    }

    /// Validates and executes movement, recording its route, resource use, city changes, and reserve status.
    pub(super) fn move_unit(
        &mut self,
        unit_id: UnitId,
        destination: HexId,
        mode: MovementMode,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let (side_id, phase) = self.movement_phase()?;
        let unit = self.owned_unit(&unit_id, &side_id)?.clone();
        let (path, cost) = self.plan_movement(&side_id, &unit, &destination, mode, phase)?;
        let from = unit.location.clone();

        self.units
            .get_mut(&unit_id)
            .expect("validated unit exists")
            .location = UnitLocation::Hex {
            hex_id: destination.clone(),
        };
        let mut city_events = Vec::new();
        // 30.1.1: only Tactical movement may enter enemy-controlled Conquered Cities.
        let city_control_changes = if mode == MovementMode::Tactical {
            self.take_cities_along(&side_id, &path, &mut city_events)
        } else {
            Vec::new()
        };
        let movement = PlannedMovement {
            unit_id: unit_id.clone(),
            from: from.clone(),
            to: destination.clone(),
            mode,
            cost,
            path: path.clone(),
            city_control_changes,
        };
        match mode {
            MovementMode::AirTransport | MovementMode::Paradrop => {
                self.active_plan_mut()?.airlift_steps_used += unit.step_count();
            }
            MovementMode::SeaTransport => {
                self.active_plan_mut()?.sealift_steps_used += unit.step_count();
            }
            _ => {}
        }
        self.phase_movements_mut(phase)?.push(movement);

        let mut events = vec![GameEvent::UnitMoved {
            unit_id: unit_id.clone(),
            from,
            to: destination,
            mode,
            cost,
            path,
        }];
        events.extend(city_events);
        if phase == MovementPhase::Planning {
            self.recheck_reserve(&unit_id, &mut events);
        }
        Ok(events)
    }

    /// Validates a rail-loading order and marks the unit as Entraining for a later player turn.
    pub(super) fn entrain_unit(&mut self, unit_id: UnitId) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.battle_planning_side()?;
        let unit = self.owned_unit(&unit_id, &side_id)?.clone();
        // House rule: units leave the Strategic Reserve by air or sea transport,
        // or arrive by rail at their sector's entry hex, never by entraining there.
        if unit.location == UnitLocation::StrategicReserve {
            return Err(RuleError::new(
                "unitOffMap",
                "Units in the Strategic Reserve leave it by air or sea transport, not by rail",
            ));
        }
        if unit.train_status.is_some() {
            return Err(RuleError::new(
                "unitAlreadyUnderTrainMarker",
                "The unit is already entraining or entrained",
            ));
        }
        if unit.supply.movement == Some(SupplyStatus::OutOfSupply)
            || unit.supply.headquarters == Some(SupplyStatus::OutOfSupply)
        {
            return Err(RuleError::new(
                "unitOutOfSupply",
                "An out-of-supply unit may not entrain",
            ));
        }
        if unit.has_trait("immobile") {
            return Err(RuleError::new(
                "unitImmobile",
                "The scenario does not allow this unit to move",
            ));
        }
        // 13.1 (3), 25.6.4 (4): Disrupted or Suppressed units may not entrain.
        if unit.disruption.is_some() {
            return Err(RuleError::new(
                "unitDisrupted",
                "A Disrupted or Suppressed unit may not entrain",
            ));
        }
        // 13.1 (2): entraining requires friendly Airspace.
        if let UnitLocation::Hex { hex_id } = &unit.location {
            if self.airspace_map(&side_id).of(&hex_id.0) != Airspace::Friendly {
                return Err(RuleError::new(
                    "notFriendlyAirspace",
                    "A unit may entrain only in friendly Airspace",
                ));
            }
        }
        if !self
            .active_plan()?
            .movements
            .iter()
            .all(|movement| movement.unit_id != unit_id)
        {
            return Err(RuleError::new(
                "unitAlreadyMoved",
                "A unit must spend the full planning phase entraining",
            ));
        }
        if let UnitLocation::Hex { hex_id } = &unit.location {
            if self.hex_in_enemy_zoc(&side_id, hex_id) {
                return Err(RuleError::new(
                    "enemyZoneOfControl",
                    "A unit in an enemy zone of control may not entrain",
                ));
            }
        }

        // 13.3 (living rules): Entraining units do not count against rail
        // capacity; the limit is checked when the marker flips to Entrained.
        self.units
            .get_mut(&unit_id)
            .expect("validated unit exists")
            .train_status = Some(TrainStatus::Entraining);
        let plan = self.active_plan_mut()?;
        plan.detrained_unit_ids.retain(|id| id != &unit_id);
        if !plan.entraining_unit_ids.contains(&unit_id) {
            plan.entraining_unit_ids.push(unit_id.clone());
        }
        let mut events = vec![GameEvent::TrainStatusChanged {
            unit_id: unit_id.clone(),
            status: Some(TrainStatus::Entraining),
        }];
        self.recheck_reserve(&unit_id, &mut events);
        Ok(events)
    }

    /// Removes an eligible unit's train marker and records the detrain order for possible undo.
    pub(super) fn detrain_unit(&mut self, unit_id: UnitId) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.battle_planning_side()?;
        let unit = self.owned_unit(&unit_id, &side_id)?;
        let previous_status = unit.train_status;
        if previous_status.is_none() {
            return Err(RuleError::new(
                "unitNotUnderTrainMarker",
                "The unit is not entraining or entrained",
            ));
        }
        self.units
            .get_mut(&unit_id)
            .expect("validated unit exists")
            .train_status = None;
        let plan = self.active_plan_mut()?;
        let cancelled_current_order = previous_status == Some(TrainStatus::Entraining)
            && plan.entraining_unit_ids.contains(&unit_id);
        plan.entraining_unit_ids.retain(|id| id != &unit_id);
        if cancelled_current_order {
            plan.detrained_unit_ids.retain(|id| id != &unit_id);
        } else if !plan.detrained_unit_ids.contains(&unit_id) {
            plan.detrained_unit_ids.push(unit_id.clone());
        }
        Ok(vec![GameEvent::TrainStatusChanged {
            unit_id,
            status: None,
        }])
    }

    /// Restores a train marker for this plan's detrain order after checking movement and rail capacity.
    pub(super) fn undo_detrain_unit(
        &mut self,
        unit_id: UnitId,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.battle_planning_side()?;
        let unit = self.owned_unit(&unit_id, &side_id)?.clone();
        let plan = self.active_plan()?;
        if !plan.detrained_unit_ids.contains(&unit_id) || unit.train_status.is_some() {
            return Err(RuleError::new(
                "unitNotDetrainedThisPlan",
                "Only a detrain order given during this battle plan can be undone",
            ));
        }
        if plan
            .movements
            .iter()
            .any(|movement| movement.unit_id == unit_id && movement.mode != MovementMode::Rail)
        {
            return Err(RuleError::new(
                "unitAlreadyMoved",
                "Undo the unit's non-rail movement before restoring its train marker",
            ));
        }
        self.validate_rail_capacity(&side_id, &unit)?;

        self.units
            .get_mut(&unit_id)
            .expect("validated unit exists")
            .train_status = Some(TrainStatus::Entrained);
        self.active_plan_mut()?
            .detrained_unit_ids
            .retain(|id| id != &unit_id);
        Ok(vec![GameEvent::TrainStatusChanged {
            unit_id,
            status: Some(TrainStatus::Entrained),
        }])
    }

    /// Reverses a unit's last phase movement, restoring its location, city control, and airlift use.
    pub(super) fn undo_unit_movement(
        &mut self,
        unit_id: UnitId,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let (side_id, phase) = self.movement_phase()?;
        let unit = self.owned_unit(&unit_id, &side_id)?.clone();
        let movement_index = self
            .phase_movements(phase)?
            .iter()
            .rposition(|movement| movement.unit_id == unit_id)
            .ok_or_else(|| {
                RuleError::new(
                    "movementNotFound",
                    "This unit has no movement order to undo",
                )
            })?;
        let movement = self.phase_movements(phase)?[movement_index].clone();
        if !matches!(&unit.location, UnitLocation::Hex { hex_id } if hex_id == &movement.to) {
            return Err(RuleError::new(
                "movementUndoConflict",
                "The unit is no longer at the endpoint of that movement",
            ));
        }
        if let UnitLocation::Hex { hex_id } = &movement.from {
            self.validate_destination_stacking(&unit, hex_id)?;
        }

        self.units
            .get_mut(&unit_id)
            .expect("validated unit exists")
            .location = movement.from.clone();
        self.phase_movements_mut(phase)?.remove(movement_index);
        match movement.mode {
            MovementMode::AirTransport | MovementMode::Paradrop => {
                let plan = self.active_plan_mut()?;
                plan.airlift_steps_used = plan.airlift_steps_used.saturating_sub(unit.step_count());
            }
            MovementMode::SeaTransport => {
                let plan = self.active_plan_mut()?;
                plan.sealift_steps_used = plan.sealift_steps_used.saturating_sub(unit.step_count());
            }
            _ => {}
        }
        let mut city_events = Vec::new();
        self.restore_city_control(&movement.city_control_changes, &mut city_events);
        let mut events = vec![GameEvent::UnitMovementUndone {
            restored_location: movement.from.clone(),
            movement,
        }];
        events.extend(city_events);
        if phase == MovementPhase::Planning {
            self.recheck_reserve(&unit_id, &mut events);
        }
        Ok(events)
    }

    /// Completes train loading within rail capacity and creates a fresh plan for the acting side.
    pub(super) fn start_battle_plan(&mut self, events: &mut Vec<GameEvent>) {
        let Some(side_id) = self.current_step().and_then(|step| match &step.actor {
            PhaseActor::Side { side_id } => Some(side_id.clone()),
            PhaseActor::All => None,
        }) else {
            return;
        };
        // 13.1, 13.3: Entraining units flip to Entrained while rail capacity
        // allows, in unit-ID order; the rest keep their Entraining marker.
        let entraining: Vec<UnitId> = self
            .units
            .values()
            .filter(|unit| {
                unit.definition.side_id == side_id
                    && unit.train_status == Some(TrainStatus::Entraining)
            })
            .map(|unit| unit.id().clone())
            .collect();
        for unit_id in entraining {
            let unit = &self.units[&unit_id];
            if self.validate_rail_capacity(&side_id, unit).is_err() {
                continue;
            }
            self.units
                .get_mut(&unit_id)
                .expect("unit exists")
                .train_status = Some(TrainStatus::Entrained);
            events.push(GameEvent::TrainStatusChanged {
                unit_id,
                status: Some(TrainStatus::Entrained),
            });
        }
        self.battle_plan = Some(BattlePlan::new(self.game_turn, side_id));
    }

    /// Removes Disrupted markers and supplies friendly combat units in the selected target stacks.
    pub(super) fn finish_battle_plan(&mut self, events: &mut Vec<GameEvent>) {
        let Some(plan) = &self.battle_plan else {
            return;
        };
        // Recovery (7.2 C) follows movement: the side's Disrupted markers come off.
        let side_id = plan.side_id.clone();
        self.remove_disruption(&side_id, events);
        let Some(plan) = &self.battle_plan else {
            return;
        };
        let side_id = plan.side_id.clone();
        let target_ids = plan.resupply_target_unit_ids.clone();
        let mut resupplied_hex_ids = Vec::new();
        for target_id in target_ids {
            let Some(UnitLocation::Hex { hex_id }) =
                self.units.get(&target_id).map(|unit| unit.location.clone())
            else {
                continue;
            };
            // Two selected units may have moved into the same stack. Applying
            // resupply once is sufficient and avoids duplicate events.
            if resupplied_hex_ids.contains(&hex_id) {
                continue;
            }
            resupplied_hex_ids.push(hex_id.clone());

            let mut unit_ids = Vec::new();
            for unit in self.units.values_mut().filter(|unit| {
                unit.definition.side_id == side_id
                    && !unit.is_headquarters()
                    && matches!(&unit.location, UnitLocation::Hex { hex_id: occupied } if occupied == &hex_id)
            }) {
                unit.mark_fully_supplied();
                unit_ids.push(unit.id().clone());
            }
            events.push(GameEvent::UnitsResupplied { hex_id, unit_ids });
        }
    }

    /// Returns the acting side after verifying that a single-side Battle Planning Phase is active.
    pub(crate) fn battle_planning_side(&self) -> Result<SideId, RuleError> {
        let Some(step) = self.current_step() else {
            return Err(crate::error::game_complete());
        };
        if step.phase_id.0 != "battlePlanning" {
            return Err(RuleError::new(
                "wrongPhase",
                "This command is available only during Battle Planning",
            ));
        }
        match &step.actor {
            PhaseActor::Side { side_id } => Ok(side_id.clone()),
            PhaseActor::All => Err(RuleError::new(
                "wrongActor",
                "Battle Planning must have one acting side",
            )),
        }
    }

    /// Borrows the active battle plan or reports that no plan exists.
    pub(crate) fn active_plan(&self) -> Result<&BattlePlan, RuleError> {
        self.battle_plan
            .as_ref()
            .ok_or_else(|| RuleError::new("battlePlanUnavailable", "No active battle plan exists"))
    }

    /// Mutably borrows the active battle plan or reports that no plan exists.
    pub(crate) fn active_plan_mut(&mut self) -> Result<&mut BattlePlan, RuleError> {
        self.battle_plan
            .as_mut()
            .ok_or_else(|| RuleError::new("battlePlanUnavailable", "No active battle plan exists"))
    }

    /// Looks up a unit and verifies that the specified side controls it.
    pub(crate) fn owned_unit(
        &self,
        unit_id: &UnitId,
        side_id: &SideId,
    ) -> Result<&UnitState, RuleError> {
        let unit = self
            .units
            .get(unit_id)
            .ok_or_else(|| RuleError::new("unknownUnit", format!("Unknown unit: {}", unit_id.0)))?;
        if unit.definition.side_id != *side_id {
            return Err(RuleError::new(
                "unitNotControlled",
                "The acting side does not control this unit",
            ));
        }
        Ok(unit)
    }

    /// Checks whether adding this unit's steps to entrained units would exceed its side's capacity.
    pub(crate) fn validate_rail_capacity(
        &self,
        side_id: &SideId,
        unit: &UnitState,
    ) -> Result<(), RuleError> {
        let capacity = if side_id.0 == "warsawPact" {
            self.scenario
                .battle_planning_rules
                .warsaw_pact_rail_capacity
        } else {
            self.scenario.battle_planning_rules.nato_rail_capacity
        };
        let used: u16 = self
            .units
            .values()
            .filter(|other| {
                other.definition.side_id == *side_id
                    && other.train_status == Some(TrainStatus::Entrained)
            })
            .map(UnitState::step_count)
            .sum();
        if used + unit.step_count() > capacity {
            return Err(RuleError::new(
                "railCapacityExceeded",
                format!("Another Entrained unit would exceed the {capacity}-step rail capacity"),
            ));
        }
        Ok(())
    }
}
