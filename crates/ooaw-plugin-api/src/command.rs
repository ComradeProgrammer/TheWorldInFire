//! The fixed set of player commands understood by every OOAW rules plugin.

use serde::{Deserialize, Serialize};

use crate::{AirBaseId, AirUnitId, HexId, SideId, UnitId};

/// Movement system selected for an authoritative unit movement order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MovementMode {
    /// Normal ground movement using the printed movement allowance.
    Tactical,
    /// Road-column movement using twice the printed movement allowance.
    March,
    /// Movement by rail after the unit has finished entraining.
    Rail,
    /// Movement by an Airlift Command from a city to a friendly-controlled city.
    AirTransport,
    /// An Airborne unit dropped by an Airlift Command onto Clear or Marsh terrain.
    Paradrop,
    /// Movement by a Sealift Command from a port to a friendly-controlled port.
    SeaTransport,
}

/// Kind of Air Point selected for an air mission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AirPointKind {
    /// Short-range point, restricted by the rules plugin's airspace rules.
    Tactical,
    /// Long-range point.
    Operational,
}

/// Mission assigned to one named air counter during battle planning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AirMissionAssignment {
    /// Patrol centred on one map hex.
    AirSuperiority {
        /// Centre of the patrol area.
        center_hex_id: HexId,
    },
    /// Strike against specified ground units in one hex.
    GroundStrike {
        /// Target hex.
        hex_id: HexId,
        /// Target ground units.
        unit_ids: Vec<UnitId>,
    },
    /// Strike against an off-map airbase.
    AirBaseStrike {
        /// Target airbase.
        air_base_id: AirBaseId,
    },
    /// Early-warning orbit centred on one map hex.
    EarlyWarning {
        /// Centre of the early-warning area.
        center_hex_id: HexId,
    },
}

/// A player or client request that may change the authoritative game state.
///
/// This enum is the closed command vocabulary shared by the kernel and every
/// rules plugin. A plugin decides how a command is validated and applied, but
/// cannot add command names or change their payloads.
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
    /// Moves a unit using a plugin-selected and fully validated route.
    MoveUnit {
        /// Friendly unit to move.
        unit_id: UnitId,
        /// Desired destination hex.
        destination: HexId,
        /// Movement system to use.
        mode: MovementMode,
    },
    /// Adds or removes a Reserve or OMG marker during battle planning.
    SetReserve {
        /// Friendly maneuver unit.
        unit_id: UnitId,
        /// Whether the unit should carry the marker.
        selected: bool,
    },
    /// Reverses the most recent movement order for one unit.
    UndoUnitMovement {
        /// Unit whose last movement is undone.
        unit_id: UnitId,
    },
    /// Orders a stationary supplied unit to spend this turn entraining.
    EntrainUnit {
        /// Unit to place under an Entraining marker.
        unit_id: UnitId,
    },
    /// Removes a train marker so the unit may use another movement system.
    DetrainUnit {
        /// Unit whose train marker is removed.
        unit_id: UnitId,
    },
    /// Cancels a detrain order made during the current plan.
    UndoDetrainUnit {
        /// Unit whose Entrained marker is restored.
        unit_id: UnitId,
    },
    /// Commits one named air counter to its current-turn mission.
    PlanAirSortie {
        /// Air counter receiving the order.
        air_unit_id: AirUnitId,
        /// Mission and target selected during battle planning.
        mission: AirMissionAssignment,
    },
    /// Cancels one sortie in the acting side's current air plan.
    CancelAirSortie {
        /// Stable sortie identifier within the plan.
        sortie_id: u32,
    },
    /// Commits one Air Point to strike ground units in one hex.
    PlanAirStrike {
        /// Target hex.
        hex_id: HexId,
        /// Targeted enemy units.
        unit_ids: Vec<UnitId>,
        /// Kind of Air Point to spend.
        air_point: AirPointKind,
    },
    /// Commits one Air Point to place an Air Interdiction Zone.
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
    /// Resolves every committed air mission.
    ResolveAirStrikes,
    /// Commits adjacent units against an objective and resolves the battle.
    ResolveBattle {
        /// Objective hex.
        hex_id: HexId,
        /// Committed attacking units.
        unit_ids: Vec<UnitId>,
        /// Headquarters committing offensive support, if any.
        #[serde(default)]
        supporting_hq_id: Option<UnitId>,
    },
    /// Advances surviving attackers into a cleared objective.
    AdvanceAfterCombat {
        /// Units that advance; an empty list declines the advance.
        unit_ids: Vec<UnitId>,
    },
    /// Debug command that checks one side's supply immediately.
    #[serde(rename = "debug.checkSupply")]
    DebugCheckSupply {
        /// Side whose units are checked.
        side_id: SideId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serde_json::{self, json};

    #[test]
    fn commands_use_the_shared_camel_case_wire_shape() {
        let command: GameCommand = serde_json::from_value(json!({
            "type": "moveUnit",
            "unitId": "unit-1",
            "destination": "2417",
            "mode": "tactical"
        }))
        .unwrap();
        assert_eq!(
            command,
            GameCommand::MoveUnit {
                unit_id: UnitId("unit-1".to_owned()),
                destination: HexId("2417".to_owned()),
                mode: MovementMode::Tactical,
            }
        );
    }

    #[test]
    fn plugins_cannot_introduce_unknown_commands() {
        let error = serde_json::from_value::<GameCommand>(json!({
            "type": "teleportUnit",
            "unitId": "unit-1"
        }))
        .unwrap_err();
        assert!(error.to_string().contains("unknown variant"));
    }
}
