use serde::{Deserialize, Serialize};

use crate::model::{AirPointKind, HexId, MovementMode, SideId, UnitId};

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
    /// Adds or removes a stack from this turn's resupply operations.
    SetResupplyTarget {
        /// Friendly combat unit identifying the stack.
        unit_id: UnitId,
        /// Whether the stack should be present in the plan.
        selected: bool,
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
    /// Adds or removes a Reserve (NATO) or OMG (WP) Marker during Battle Planning (12.6).
    SetReserve {
        /// Friendly Maneuver unit.
        unit_id: UnitId,
        /// Whether the unit should carry the marker.
        selected: bool,
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
    /// Commits adjacent units against an Objective hex and resolves the battle (25.1).
    ResolveBattle {
        /// Objective hex.
        hex_id: HexId,
        /// Committed attacking units.
        unit_ids: Vec<UnitId>,
        /// HQ committing Offensive Support (25.4), if any.
        #[serde(default)]
        supporting_hq_id: Option<UnitId>,
    },
    /// Advances surviving attackers into the cleared Objective hex; an empty
    /// list declines the advance (25.8.1).
    AdvanceAfterCombat {
        /// Units that advance.
        unit_ids: Vec<UnitId>,
    },
    /// Debug command: checks one side's supply immediately, as the Pre-Battle
    /// Phase does. Accepted only when the kernel enables debug commands.
    #[serde(rename = "debug.checkSupply")]
    DebugCheckSupply {
        /// Side whose units are checked.
        side_id: SideId,
    },
}
