//! Custom starting situations for tests, puzzles, and debugging.
//!
//! A [`GameSetup`] starts a scenario normally, advances through its turn
//! sequence to a chosen step (resolving every phase on the way, including
//! reinforcements and supply), and then lays out the requested situation:
//! the units in play, their condition, city control, and markers.
//!
//! The situation replaces state; it is not re-validated against the rules.
//! Automatic work of the starting step itself (such as the Pre-Battle supply
//! check) has already run, so to have the engine check supply for a setup,
//! start one step earlier and submit `endPhase`.

use serde::{Deserialize, Serialize};

use crate::error::RuleError;
use crate::model::{
    AirInterdictionZone, AirPoints, Disruption, HexId, PhaseId, SideId, SupplyStatus, TrainStatus,
    UnitId, UnitLocation, UnitState, UnitSupplyState,
};
use crate::rules::Rules;

/// A custom starting situation applied to a new game.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct GameSetup {
    /// Step to start from; the game's first step when omitted.
    pub start: Option<SetupStart>,
    /// Units in play, replacing every unit present at the start step. Omit to
    /// keep the scenario's units as they are at that step.
    pub units: Option<Vec<SetupUnit>>,
    /// City control overrides; other cities keep their control.
    pub city_control: Vec<SetupCity>,
    /// Breakthrough Markers, replacing any present.
    pub breakthrough_markers: Option<Vec<HexId>>,
    /// Air Interdiction Zones, replacing any present.
    pub air_interdiction_zones: Option<Vec<AirInterdictionZone>>,
    /// Air Points, replacing those of the listed sides.
    pub air_points: Vec<AirPoints>,
    /// Attack objectives of the active battle plan (requires one).
    pub attack_targets: Option<Vec<HexId>>,
    /// Units under a Reserve/OMG Marker in the active battle plan (requires one).
    pub reserve_unit_ids: Option<Vec<UnitId>>,
}

/// The turn-sequence step a setup starts from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupStart {
    /// One-based game turn.
    pub game_turn: u16,
    /// Phase type, such as `battlePlanning` or `combat`.
    pub phase_id: PhaseId,
    /// Acting side; omit for joint phases.
    #[serde(default)]
    pub side_id: Option<SideId>,
}

/// One unit placed by a setup. Its definition comes from the scenario, so any
/// scenario unit may be placed, including reinforcements not yet arrived.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupUnit {
    /// Scenario unit identifier.
    pub id: UnitId,
    /// Map hex; omit to place the unit in the Strategic Reserve.
    #[serde(default)]
    pub hex: Option<HexId>,
    /// Index of the active strength step; full strength when omitted.
    #[serde(default)]
    pub step: usize,
    /// Disrupted or Suppressed marker.
    #[serde(default)]
    pub disruption: Option<Disruption>,
    /// Supply of every type that applies to the unit; supplied when omitted.
    #[serde(default)]
    pub supply: Option<SupplyStatus>,
    /// Train marker.
    #[serde(default)]
    pub train_status: Option<TrainStatus>,
}

/// A city control override.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupCity {
    /// City hex.
    pub hex_id: HexId,
    /// Side that controls it.
    pub controller: SideId,
}

fn invalid(message: impl Into<String>) -> RuleError {
    RuleError::new("invalidSetup", message)
}

impl Rules {
    /// Replaces the state at the setup's start step with the requested situation.
    ///
    /// The kernel has already played to the start step.
    pub(crate) fn apply_setup(&mut self, setup: &GameSetup) -> Result<(), RuleError> {
        let known_hex =
            |hex_id: &HexId| self.scenario.map.hexes.iter().any(|hex| hex.id == *hex_id);
        if let Some(units) = &setup.units {
            let mut placed = std::collections::BTreeMap::new();
            for entry in units {
                let mut unit = self
                    .scenario
                    .reinforcements
                    .iter()
                    .find(|reinforcement| reinforcement.unit.id() == &entry.id)
                    .map(|reinforcement| reinforcement.unit.clone())
                    .ok_or_else(|| invalid(format!("Unknown unit: {}", entry.id.0)))?;
                if entry.step >= unit.definition.steps.len() {
                    return Err(invalid(format!(
                        "{} has no step {}",
                        entry.id.0, entry.step
                    )));
                }
                unit.location = match &entry.hex {
                    Some(hex_id) if known_hex(hex_id) => UnitLocation::Hex {
                        hex_id: hex_id.clone(),
                    },
                    Some(hex_id) => return Err(invalid(format!("Unknown hex: {}", hex_id.0))),
                    None => UnitLocation::StrategicReserve,
                };
                unit.strength_step_index = entry.step;
                unit.disruption = entry.disruption;
                unit.train_status = entry.train_status;
                unit.supply = setup_supply(&unit, entry.supply.unwrap_or(SupplyStatus::Supplied));
                if placed.insert(entry.id.clone(), unit).is_some() {
                    return Err(invalid(format!("{} is listed twice", entry.id.0)));
                }
            }
            self.units = placed;
        }

        for city in &setup.city_control {
            let Some(control) = self.city_control.get_mut(&city.hex_id) else {
                return Err(invalid(format!("No city in hex {}", city.hex_id.0)));
            };
            *control = city.controller.clone();
        }
        if let Some(markers) = &setup.breakthrough_markers {
            self.breakthrough_markers = markers.clone();
        }
        if let Some(zones) = &setup.air_interdiction_zones {
            self.air_interdiction_zones = zones.clone();
        }
        for points in &setup.air_points {
            let Some(current) = self
                .air_points
                .iter_mut()
                .find(|current| current.side_id == points.side_id)
            else {
                return Err(invalid(format!("Unknown side: {}", points.side_id.0)));
            };
            *current = points.clone();
        }

        if setup.attack_targets.is_some() || setup.reserve_unit_ids.is_some() {
            let plan = self.battle_plan.as_mut().ok_or_else(|| {
                invalid("Attack targets and reserves need a battle plan: start at or after Battle Planning")
            })?;
            if let Some(targets) = &setup.attack_targets {
                plan.attack_targets = targets.clone();
            }
            if let Some(reserves) = &setup.reserve_unit_ids {
                plan.reserve_unit_ids = reserves.clone();
            }
        }
        // The Reserve Phase lists the marked units when it opens; reopen it so
        // the setup's units and markers are used.
        if self.reserve.is_some() {
            self.start_reserve();
        }
        Ok(())
    }
}

/// `status` for every supply type that applies to the unit.
fn setup_supply(unit: &UnitState, status: SupplyStatus) -> UnitSupplyState {
    if unit.is_headquarters() {
        UnitSupplyState {
            headquarters: Some(status),
            movement: None,
            combat: None,
        }
    } else {
        UnitSupplyState {
            headquarters: None,
            movement: Some(status),
            combat: Some(status),
        }
    }
}
