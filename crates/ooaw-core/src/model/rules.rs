use serde::{Deserialize, Serialize};

use super::{AirPowerRules, MapHex, SideAirPower, SideId, Terrain};

/// Scenario-controlled rules used while constructing a battle plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattlePlanningRules {
    /// Number of hexes an entrained unit may travel in one player turn.
    pub rail_movement_limit: u16,
    /// Maximum Warsaw Pact steps that may be under a train marker.
    pub warsaw_pact_rail_capacity: u16,
    /// Maximum NATO steps that may be under a train marker.
    pub nato_rail_capacity: u16,
    /// Warsaw Pact Airlift Commands; each carries one step per game turn (3.8).
    pub warsaw_pact_airlift_commands: u16,
    /// NATO Airlift Commands; each carries one step per game turn (3.8).
    pub nato_airlift_commands: u16,
    /// Resupply operations available to each side in one player turn.
    pub resupply_operations_per_turn: u16,
    /// Maximum maneuver-unit steps in one hex at the end of movement.
    pub maneuver_stacking_limit: u16,
    /// Additional movement points to cross a major-river hexside.
    pub major_river_cost: u16,
    /// Additional movement points to cross a minor-river hexside.
    pub minor_river_cost: u16,
    /// Air Points and Surprise (23.1, 35.7).
    pub air_power: AirPowerRules,
}

impl BattlePlanningRules {
    /// Rules used by the built-in NATO scenarios.
    pub fn nato_standard() -> Self {
        Self {
            rail_movement_limit: 20,
            warsaw_pact_rail_capacity: 8,
            nato_rail_capacity: 10,
            warsaw_pact_airlift_commands: 1,
            nato_airlift_commands: 1,
            resupply_operations_per_turn: 1,
            maneuver_stacking_limit: 4,
            // Rule 12.1 example: crossing a Major River costs one extra point.
            major_river_cost: 1,
            // The TEC card is not in the reference material; minor rivers are
            // treated as free to cross until its value is confirmed.
            minor_river_cost: 0,
            // Placeholder for the rules prototype; real scenarios roll on an Air
            // Campaign Table, which is not implemented.
            air_power: AirPowerRules {
                warsaw_pact: SideAirPower {
                    tactical_per_turn: 2,
                    operational_per_turn: 1,
                    bonus_tactical: 0,
                },
                nato: SideAirPower {
                    tactical_per_turn: 2,
                    operational_per_turn: 1,
                    bonus_tactical: 0,
                },
                surprise_turn: None,
            },
        }
    }

    /// Airlift Commands, and therefore airlift steps per turn, for one side.
    pub fn airlift_commands(&self, side_id: &SideId) -> u16 {
        if side_id.0 == "warsawPact" {
            self.warsaw_pact_airlift_commands
        } else {
            self.nato_airlift_commands
        }
    }

    /// Returns the ground movement-point cost to enter a hex's primary terrain.
    ///
    /// A city outranks the natural terrain beneath it (rule 2.2.1) and costs one
    /// point to enter (rule 12.3 example).
    pub fn hex_entry_cost(&self, hex: &MapHex) -> Option<u16> {
        if hex.terrain == Terrain::Sea {
            return None;
        }
        if hex.city.is_some() {
            return Some(1);
        }
        self.terrain_cost(hex.terrain)
    }

    /// Returns the ground movement-point cost to enter a terrain type.
    pub fn terrain_cost(&self, terrain: Terrain) -> Option<u16> {
        match terrain {
            Terrain::Sea => None,
            // Designer's Note to 25.8.2 (living rules): Marsh moves like Clear and
            // differs only by blocking Exploitation.
            Terrain::Clear | Terrain::Forest | Terrain::Marsh => Some(1),
            Terrain::Rough => Some(2),
            Terrain::Mountain => Some(3),
        }
    }
}

/// Mode used for an authoritative unit movement order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MovementMode {
    /// Normal ground movement using the printed movement allowance.
    Tactical,
    /// Road-column movement using twice the printed movement allowance.
    March,
    /// Movement by rail after the unit has finished entraining.
    Rail,
    /// Movement by an Airlift Command.
    AirTransport,
}

/// Persistent status of a unit using rail transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrainStatus {
    /// The unit is spending the current player turn loading onto trains.
    Entraining,
    /// The unit has completed loading and may move by rail.
    Entrained,
}
