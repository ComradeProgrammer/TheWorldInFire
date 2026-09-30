//! Reserve/OMG status (12.6) and the Reserve Phase (28.0).
//!
//! During Battle Planning a Maneuver unit that has moved at most half its
//! Movement Allowance may be marked Reserve (NATO) or OMG (WP). It may not
//! attack, but in the Reserve Phase after combat it moves again by Tactical
//! movement with half its printed allowance.

use crate::engine::GameEngine;
use crate::error::RuleError;
use crate::event::GameEvent;
use crate::model::{
    movement_spent, MovementMode, PhaseActor, PlannedMovement, ReserveOption, ReserveState, SideId,
    SupplyStatus, UnitId, UnitLocation, UnitState,
};

/// The phase a movement order belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MovementPhase {
    /// Battle Planning, which includes the original Movement Phase.
    Planning,
    /// The Reserve Phase: marked units only, Tactical movement at half allowance.
    Reserve,
}

impl GameEngine {
    /// Selects or deselects an owned unit's Reserve/OMG marker after checking eligibility.
    pub(super) fn set_reserve(
        &mut self,
        unit_id: UnitId,
        selected: bool,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.battle_planning_side()?;
        let unit = self.owned_unit(&unit_id, &side_id)?;
        if selected {
            self.reserve_eligibility(&side_id, unit)?;
            let reserves = &mut self.active_plan_mut()?.reserve_unit_ids;
            if !reserves.contains(&unit_id) {
                reserves.push(unit_id.clone());
            }
        } else {
            self.active_plan_mut()?
                .reserve_unit_ids
                .retain(|id| id != &unit_id);
        }
        Ok(vec![GameEvent::ReserveStatusChanged {
            side_id,
            unit_id,
            selected,
        }])
    }

    /// Previews Reserve/OMG marker eligibility for every unit of the planning side.
    ///
    /// Checks unit type, supply, disruption, train status, movement already spent,
    /// and enemy zones of control. `SetReserve` rechecks these rules on submission.
    ///
    /// # Returns
    ///
    /// Entries in unit-identifier order. Each [`ReserveOption`] has `unavailable`
    /// set to its rule error when that unit cannot currently be marked.
    ///
    /// # Errors
    ///
    /// Returns a [`RuleError`] if the game has ended or the current step is not
    /// a Battle Planning Phase with one acting side. Unit-specific failures are
    /// returned within the entries.
    pub fn reserve_options(&self) -> Result<Vec<ReserveOption>, RuleError> {
        let side_id = self.battle_planning_side()?;
        Ok(self
            .units
            .values()
            .filter(|unit| unit.definition.side_id == side_id)
            .map(|unit| ReserveOption {
                unit_id: unit.id().clone(),
                unavailable: self.reserve_eligibility(&side_id, unit).err(),
            })
            .collect())
    }

    /// 12.6 (living rules, 1 Jan 2026): the conditions for a Reserve/OMG Marker.
    fn reserve_eligibility(&self, side_id: &SideId, unit: &UnitState) -> Result<(), RuleError> {
        if unit.is_headquarters() {
            return Err(RuleError::new(
                "notManeuverUnit",
                "Only Maneuver units can be placed in reserve",
            ));
        }
        let UnitLocation::Hex { hex_id } = &unit.location else {
            return Err(RuleError::new(
                "unitOffMap",
                "Only units on the map can be placed in reserve",
            ));
        };
        if unit.has_trait("immobile") {
            return Err(RuleError::new(
                "unitImmobile",
                "The scenario does not allow this unit to move",
            ));
        }
        // (4)
        if unit.disruption.is_some() {
            return Err(RuleError::new(
                "unitDisrupted",
                "A Disrupted unit cannot be placed in reserve",
            ));
        }
        // (3)
        if unit.supply.movement == Some(SupplyStatus::OutOfSupply)
            || unit.supply.combat == Some(SupplyStatus::OutOfSupply)
        {
            return Err(RuleError::new(
                "unitOutOfSupply",
                "A unit that is out of supply cannot be placed in reserve",
            ));
        }
        // 28.1 (4): no train marker.
        if unit.train_status.is_some() {
            return Err(RuleError::new(
                "unitUnderTrainMarker",
                "A unit under a train marker cannot be placed in reserve",
            ));
        }
        let movements = &self.active_plan()?.movements;
        // (1): rail and air transport are earlier Movement Segments.
        if movements.iter().any(|movement| {
            movement.unit_id == *unit.id()
                && matches!(
                    movement.mode,
                    MovementMode::Rail | MovementMode::AirTransport
                )
        }) {
            return Err(RuleError::new(
                "movedByTransport",
                "A unit that moved by rail or air transport cannot be placed in reserve",
            ));
        }
        // (2): at most half the Movement Allowance of the system used, rounded
        // down. Minimum movement counts as the whole allowance (28.1).
        let printed = unit.current_step().map_or(0, |step| step.movement);
        let march = movement_spent(movements, unit.id(), MovementMode::March);
        let (spent, allowance) = if march > 0 {
            (march, printed.saturating_mul(2))
        } else {
            (
                movement_spent(movements, unit.id(), MovementMode::Tactical),
                printed,
            )
        };
        let limit = allowance / 2;
        if spent > limit {
            return Err(RuleError::new(
                "movedTooFar",
                format!(
                    "The unit spent {spent} of its {allowance} Movement Points; reserve status allows at most {limit}"
                ),
            ));
        }
        // (5): it may start in an EZOC but not end its movement in one.
        if self.hex_in_enemy_zoc(side_id, hex_id) {
            return Err(RuleError::new(
                "enemyZoneOfControl",
                "A unit in an enemy zone of control cannot be placed in reserve",
            ));
        }
        Ok(())
    }

    /// Removes a unit's Reserve/OMG marker when movement, entraining, or undo makes it ineligible.
    pub(crate) fn recheck_reserve(&mut self, unit_id: &UnitId, events: &mut Vec<GameEvent>) {
        // Removes the marker from a unit that no longer qualifies: it moved
        // further, entrained, or an undo returned it into an enemy zone of control.
        let Some(plan) = &self.battle_plan else {
            return;
        };
        if !plan.reserve_unit_ids.contains(unit_id) {
            return;
        }
        let side_id = plan.side_id.clone();
        let eligible = self
            .units
            .get(unit_id)
            .is_some_and(|unit| self.reserve_eligibility(&side_id, unit).is_ok());
        if eligible {
            return;
        }
        if let Some(plan) = &mut self.battle_plan {
            plan.reserve_unit_ids.retain(|id| id != unit_id);
        }
        events.push(GameEvent::ReserveStatusChanged {
            side_id,
            unit_id: unit_id.clone(),
            selected: false,
        });
    }

    /// 12.6: no unit under a Reserve/OMG Marker can participate in a combat.
    pub(crate) fn is_reserve_unit(&self, unit_id: &UnitId) -> bool {
        self.battle_plan
            .as_ref()
            .is_some_and(|plan| plan.reserve_unit_ids.contains(unit_id))
    }

    /// Opens the Reserve Phase for the acting side's surviving marked units.
    pub(crate) fn start_reserve(&mut self) {
        let Some(PhaseActor::Side { side_id }) = self.current_step().map(|step| step.actor.clone())
        else {
            return;
        };
        let unit_ids = self
            .battle_plan
            .as_ref()
            .filter(|plan| plan.side_id == side_id)
            .map(|plan| {
                plan.reserve_unit_ids
                    .iter()
                    .filter(|id| {
                        self.units
                            .get(id)
                            .is_some_and(|unit| matches!(unit.location, UnitLocation::Hex { .. }))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        self.reserve = Some(ReserveState {
            side_id,
            unit_ids,
            movements: Vec::new(),
        });
    }

    /// 28.2.5: at the end of the Reserve Phase all Reserve/OMG Markers come off.
    pub(crate) fn remove_reserve_markers(&mut self, events: &mut Vec<GameEvent>) {
        let Some(reserve) = self.reserve.take() else {
            return;
        };
        if let Some(plan) = &mut self.battle_plan {
            plan.reserve_unit_ids.clear();
        }
        if !reserve.unit_ids.is_empty() {
            events.push(GameEvent::ReserveMarkersRemoved {
                side_id: reserve.side_id,
                unit_ids: reserve.unit_ids,
            });
        }
    }

    /// Borrows the active reserve movement state or reports that no Reserve Phase is active.
    pub(crate) fn reserve_state(&self) -> Result<&ReserveState, RuleError> {
        self.reserve
            .as_ref()
            .ok_or_else(|| RuleError::new("reserveUnavailable", "No Reserve Phase is active"))
    }

    /// The acting side and phase for a movement order or preview.
    pub(crate) fn movement_phase(&self) -> Result<(SideId, MovementPhase), RuleError> {
        let step = self.current_step().ok_or_else(RuleError::game_complete)?;
        let phase = match step.phase_id.0.as_str() {
            "battlePlanning" => MovementPhase::Planning,
            "reserve" => MovementPhase::Reserve,
            _ => {
                return Err(RuleError::new(
                    "wrongPhase",
                    "Units move only during Battle Planning or the Reserve Phase",
                ))
            }
        };
        match &step.actor {
            PhaseActor::Side { side_id } => Ok((side_id.clone(), phase)),
            PhaseActor::All => Err(RuleError::new(
                "wrongActor",
                "Movement requires one acting side",
            )),
        }
    }

    /// Movement orders already executed in the given phase.
    pub(crate) fn phase_movements(
        &self,
        phase: MovementPhase,
    ) -> Result<&[PlannedMovement], RuleError> {
        Ok(match phase {
            MovementPhase::Planning => &self.active_plan()?.movements,
            MovementPhase::Reserve => &self.reserve_state()?.movements,
        })
    }

    /// Mutably borrows movement orders for the requested planning or reserve phase.
    pub(crate) fn phase_movements_mut(
        &mut self,
        phase: MovementPhase,
    ) -> Result<&mut Vec<PlannedMovement>, RuleError> {
        match phase {
            MovementPhase::Planning => Ok(&mut self.active_plan_mut()?.movements),
            MovementPhase::Reserve => self
                .reserve
                .as_mut()
                .map(|reserve| &mut reserve.movements)
                .ok_or_else(|| RuleError::new("reserveUnavailable", "No Reserve Phase is active")),
        }
    }
}
