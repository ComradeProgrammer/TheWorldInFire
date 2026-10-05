use serde::{Deserialize, Serialize};

use super::{HexId, MovementMode, SideId, UnitId, UnitLocation};
use crate::error::RuleError;

/// One authoritative movement recorded in the current battle plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedMovement {
    /// Unit that moved.
    pub unit_id: UnitId,
    /// Location before this order.
    pub from: UnitLocation,
    /// Destination after this order.
    pub to: HexId,
    /// Movement system used by the order.
    pub mode: MovementMode,
    /// Movement points or rail hexes consumed by this order.
    pub cost: u16,
    /// Ground or rail path chosen and validated by the core.
    pub path: Vec<HexId>,
    /// City hexes whose control this movement changed, for undo.
    pub city_control_changes: Vec<CityControlChange>,
}

/// A city hex whose controller changed, with the controller it replaced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CityControlChange {
    /// City hex.
    pub hex_id: HexId,
    /// Controller before the change.
    pub previous_controller: SideId,
}

/// One legal destination offered to the client as a movement preview.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovementOption {
    /// Destination hex.
    pub hex_id: HexId,
    /// Movement points or rail hexes the order would consume.
    pub cost: u16,
    /// Route chosen by the core, excluding the starting hex.
    pub path: Vec<HexId>,
}

/// Movement preview for one movement system.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovementModeOptions {
    /// Movement system.
    pub mode: MovementMode,
    /// Why the unit cannot use this system at all, if it cannot.
    pub unavailable: Option<RuleError>,
    /// Legal destinations with this system; empty when unavailable.
    pub options: Vec<MovementOption>,
}

/// Mutable plan assembled by one side during its battle-planning phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BattlePlan {
    /// Game turn to which this plan belongs.
    pub game_turn: u16,
    /// Side that owns the plan.
    pub side_id: SideId,
    /// Units identifying the stacks selected for resupply operations.
    pub resupply_target_unit_ids: Vec<UnitId>,
    /// Enemy-occupied objective hexes selected for later combat.
    pub attack_targets: Vec<HexId>,
    /// Movement orders already executed during planning.
    pub movements: Vec<PlannedMovement>,
    /// Units ordered to begin entraining during this plan.
    pub entraining_unit_ids: Vec<UnitId>,
    /// Previously entrained units ordered to detrain during this plan.
    pub detrained_unit_ids: Vec<UnitId>,
    /// Airlift capacity already consumed by this plan.
    pub airlift_steps_used: u16,
    /// Sealift capacity already consumed by this plan.
    #[serde(default)]
    pub sealift_steps_used: u16,
    /// Maneuver units marked with a Reserve (NATO) or OMG (WP) Marker (12.6).
    #[serde(default)]
    pub reserve_unit_ids: Vec<UnitId>,
}

/// The acting side's Reserve Phase (28.2): marked units and their movement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveState {
    /// Side moving its reserves.
    pub side_id: SideId,
    /// Units under a Reserve/OMG Marker that may move in this phase.
    pub unit_ids: Vec<UnitId>,
    /// Movement orders executed during this phase.
    pub movements: Vec<PlannedMovement>,
}

/// Whether one friendly unit may receive a Reserve/OMG Marker now (12.6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveOption {
    /// Unit.
    pub unit_id: UnitId,
    /// Why the unit cannot be marked, if it cannot.
    pub unavailable: Option<RuleError>,
}

impl BattlePlan {
    /// Creates an empty plan for the acting side.
    pub(crate) fn new(game_turn: u16, side_id: SideId) -> Self {
        Self {
            game_turn,
            side_id,
            resupply_target_unit_ids: Vec::new(),
            attack_targets: Vec::new(),
            movements: Vec::new(),
            entraining_unit_ids: Vec::new(),
            detrained_unit_ids: Vec::new(),
            airlift_steps_used: 0,
            sealift_steps_used: 0,
            reserve_unit_ids: Vec::new(),
        }
    }
}

/// Returns the movement already spent by a unit in the indicated mode.
pub(crate) fn movement_spent(
    movements: &[PlannedMovement],
    unit_id: &UnitId,
    mode: MovementMode,
) -> u16 {
    movements
        .iter()
        .filter(|movement| movement.unit_id == *unit_id && movement.mode == mode)
        .map(|movement| movement.cost)
        .sum()
}

/// Returns whether the unit has already used a different movement system.
pub(crate) fn has_incompatible_movement(
    movements: &[PlannedMovement],
    unit_id: &UnitId,
    mode: MovementMode,
) -> bool {
    movements
        .iter()
        .any(|movement| movement.unit_id == *unit_id && movement.mode != mode)
}
