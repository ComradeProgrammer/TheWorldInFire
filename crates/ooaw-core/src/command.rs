use serde::{Deserialize, Serialize};

use crate::event::GameEvent;
use crate::model::{AirPointKind, HexId, MovementMode, UnitId};
use crate::state::GameSnapshot;

/// A player or client request that may change the authoritative game state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GameCommand {
    /// Finish the current interactive phase and advance the turn sequence.
    EndPhase,
    /// Selects or replaces the unit that will receive this turn's resupply operation.
    SetResupplyTarget {
        /// Friendly combat unit identifying the stack, or `None` to cancel.
        unit_id: Option<UnitId>,
    },
    /// Adds or removes an enemy-occupied objective from the battle plan.
    SetAttackTarget {
        /// Objective hex to change.
        hex_id: HexId,
        /// Whether the objective should be present in the plan.
        selected: bool,
    },
    /// Moves a unit using a core-selected and fully validated route.
    MoveUnit {
        /// Friendly unit to move.
        unit_id: UnitId,
        /// Desired destination hex.
        destination: HexId,
        /// Movement system to use.
        mode: MovementMode,
    },
    /// Reverses the most recent movement order for one unit.
    UndoUnitMovement {
        /// Friendly unit whose last movement is undone.
        unit_id: UnitId,
    },
    /// Orders a stationary supplied unit to spend this turn entraining.
    EntrainUnit {
        /// Friendly unit to place under an Entraining marker.
        unit_id: UnitId,
    },
    /// Removes a train marker so the unit may use another movement system.
    DetrainUnit {
        /// Friendly unit whose train marker is removed.
        unit_id: UnitId,
    },
    /// Cancels a detrain order given during the current plan, restoring the Entrained marker.
    UndoDetrainUnit {
        /// Friendly unit detrained earlier in this plan.
        unit_id: UnitId,
    },
    /// Commits one Air Point to strike up to two enemy steps in a hex (23.3).
    PlanAirStrike {
        /// Target hex.
        hex_id: HexId,
        /// Targeted enemy units; the first listed absorbs a step loss.
        unit_ids: Vec<UnitId>,
        /// Kind of Air Point to spend.
        air_point: AirPointKind,
    },
    /// Commits one Air Point to place an Air Interdiction Zone (23.8).
    PlanAirInterdiction {
        /// Marked hex.
        hex_id: HexId,
        /// Kind of Air Point to spend.
        air_point: AirPointKind,
    },
    /// Withdraws an unresolved air mission and refunds its Air Point.
    CancelAirMission {
        /// Mission to withdraw.
        mission_id: u32,
    },
    /// Resolves every committed air mission; no more may be added this phase.
    ResolveAirStrikes,
}

/// The events and current snapshot produced by an accepted game command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandOutcome {
    /// The authoritative revision after the command has been applied.
    pub revision: u64,
    /// Ordered domain events emitted while processing the command.
    pub events: Vec<GameEvent>,
    /// The complete authoritative state after command processing.
    pub snapshot: GameSnapshot,
}
