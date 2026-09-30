//! Airspace (rule 11): friendly, contested, or enemy from one side's view.

use std::collections::HashSet;

use crate::engine::GameEngine;
use crate::model::{Airspace, HexId, MapHex, SideId, SupplyStatus, UnitLocation, UnitState};

/// Hexes within this distance of a supplied unit or friendly city are contested by its side.
const AIRSPACE_RANGE: i32 = 5;

/// Calculates hex distance on the printed grid with odd rows shifted right and columns decreasing rightward.
pub(crate) fn hex_distance(a: &MapHex, b: &MapHex) -> i32 {
    // Hex distance on the printed grid (pointy-top rows, odd rows shifted right,
    // column numbers decreasing to the right).
    let axial = |hex: &MapHex| {
        let row = i32::from(hex.row);
        let x = -i32::from(hex.col);
        (x - (row - (row & 1)) / 2, row)
    };
    let (q1, r1) = axial(a);
    let (q2, r2) = axial(b);
    let (dq, dr) = (q1 - q2, r1 - r2);
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}

/// Airspace for one side, computed from the current state.
pub(crate) struct AirspaceMap {
    /// Hexes reached by the querying side's airspace sources.
    friendly: HashSet<String>,
    /// Hexes reached by the opposing side's airspace sources.
    enemy: HashSet<String>,
}

impl AirspaceMap {
    /// Classifies a hex as friendly, enemy, or contested from the cached airspace projections.
    pub(crate) fn of(&self, hex_id: &str) -> Airspace {
        match (self.friendly.contains(hex_id), self.enemy.contains(hex_id)) {
            (true, false) => Airspace::Friendly,
            (false, true) => Airspace::Enemy,
            // 11.3; a hex no side projects into is treated as contested.
            _ => Airspace::Contested,
        }
    }
}

/// Checks whether an on-map unit has the supply required to project airspace control.
fn contests_airspace(unit: &UnitState) -> bool {
    // 11.1: only units that can trace Movement Supply (HQ supply for HQs) contest.
    let supplied = if unit.is_headquarters() {
        unit.supply.headquarters == Some(SupplyStatus::Supplied)
    } else {
        unit.supply.movement == Some(SupplyStatus::Supplied)
    };
    supplied && matches!(unit.location, UnitLocation::Hex { .. })
}

impl GameEngine {
    /// Hexes within five hexes of the side's supplied units or controlled cities.
    fn contested_by(&self, side_id: &SideId) -> HashSet<String> {
        let hexes = &self.scenario.map.hexes;
        let find = |id: &HexId| hexes.iter().find(|hex| hex.id == *id);
        let mut sources: Vec<&MapHex> = self
            .units
            .values()
            .filter(|unit| unit.definition.side_id == *side_id && contests_airspace(unit))
            .filter_map(|unit| match &unit.location {
                UnitLocation::Hex { hex_id } => find(hex_id),
                UnitLocation::StrategicReserve => None,
            })
            .collect();
        // City supply is not traced yet, so every controlled city counts (11.6.1),
        // except those that never contest Airspace, such as West Berlin (11.5).
        sources.extend(
            self.city_control
                .iter()
                .filter(|(_, controller)| *controller == side_id)
                .filter_map(|(hex_id, _)| find(hex_id))
                .filter(|hex| hex.city.as_ref().is_some_and(|city| city.contests_airspace)),
        );
        hexes
            .iter()
            .filter(|hex| {
                sources
                    .iter()
                    .any(|source| hex_distance(hex, source) <= AIRSPACE_RANGE)
            })
            .map(|hex| hex.id.0.clone())
            .collect()
    }

    /// Airspace of every hex from `side_id`'s point of view.
    pub(crate) fn airspace_map(&self, side_id: &SideId) -> AirspaceMap {
        let enemy_side = self
            .scenario
            .sides
            .iter()
            .find(|side| side.id != *side_id)
            .map(|side| side.id.clone());
        AirspaceMap {
            friendly: self.contested_by(side_id),
            enemy: enemy_side
                .map(|side| self.contested_by(&side))
                .unwrap_or_default(),
        }
    }
}
