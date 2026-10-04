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
    /// Warsaw Pact Sealift Commands; each carries one step per game turn (3.8).
    pub warsaw_pact_sealift_commands: u16,
    /// NATO Sealift Commands; each carries one step per game turn (3.8).
    pub nato_sealift_commands: u16,
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
    /// Creates the default planning and movement rules for the built-in NATO scenarios.
    ///
    /// Includes rail and airlift limits, resupply and stacking allowances, river
    /// costs, and prototype Air Point allocations. Individual scenarios may adjust
    /// these values after construction.
    ///
    /// # Returns
    ///
    /// An owned rule configuration with no Surprise turn and no bonus Air Points.
    pub fn nato_standard() -> Self {
        Self {
            rail_movement_limit: 20,
            warsaw_pact_rail_capacity: 8,
            nato_rail_capacity: 10,
            warsaw_pact_airlift_commands: 1,
            nato_airlift_commands: 1,
            // 37.2, 38.2, 40.2: three Sealift Commands per side in every campaign.
            warsaw_pact_sealift_commands: 3,
            nato_sealift_commands: 3,
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

    /// Returns the configured Airlift Command capacity for one side.
    ///
    /// # Parameters
    ///
    /// - `side_id`: Side to query. `warsawPact` selects the Warsaw Pact allowance;
    ///   every other identifier selects the NATO allowance.
    ///
    /// # Returns
    ///
    /// Number of Airlift Commands, each carrying one unit step per game turn.
    /// This is the configured limit; it does not subtract commands already used.
    pub fn airlift_commands(&self, side_id: &SideId) -> u16 {
        if side_id.0 == "warsawPact" {
            self.warsaw_pact_airlift_commands
        } else {
            self.nato_airlift_commands
        }
    }

    /// Returns the configured Sealift Command capacity for one side, in steps per game turn.
    pub fn sealift_commands(&self, side_id: &SideId) -> u16 {
        if side_id.0 == "warsawPact" {
            self.warsaw_pact_sealift_commands
        } else {
            self.nato_sealift_commands
        }
    }

    /// Returns the base ground movement-point cost to enter a map hex.
    ///
    /// A city takes precedence over its underlying land terrain and costs one
    /// point to enter (rules 2.2.1 and 12.3). Sea hexes remain impassable.
    ///
    /// # Parameters
    ///
    /// - `hex`: Destination hex whose terrain and optional city determine the cost.
    ///
    /// # Returns
    ///
    /// `Some(cost)` for passable land, or `None` for sea. Hexside crossing costs,
    /// zones of control, and other route restrictions are applied separately.
    pub fn hex_entry_cost(&self, hex: &MapHex) -> Option<u16> {
        if hex.terrain == Terrain::Sea {
            return None;
        }
        if hex.city.is_some() {
            return Some(1);
        }
        self.terrain_cost(hex.terrain)
    }

    /// Returns the base ground movement-point cost for a natural terrain type.
    ///
    /// # Parameters
    ///
    /// - `terrain`: Natural terrain to price without a city override or hexside costs.
    ///
    /// # Returns
    ///
    /// One point for Clear, Forest, or Marsh; two for Rough; three for Mountain;
    /// or `None` for Sea, which cannot be entered by ground movement.
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
    /// Movement by an Airlift Command from a city to a friendly-controlled city.
    AirTransport,
    /// An Airborne unit dropped by an Airlift Command onto Clear or Marsh terrain.
    Paradrop,
    /// Movement by a Sealift Command from a port to a friendly-controlled port.
    SeaTransport,
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
