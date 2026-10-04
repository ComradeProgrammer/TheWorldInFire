//! The NATO map embedded in the official rules plugin.

use std::sync::OnceLock;

use super::{HexId, MapDefinition, ReinforcementSector, SideId};

/// Identifier of the NATO map, used when the traced data names none.
const NATO_MAP_ID: &str = "nato-central-europe";

/// Returns the NATO map embedded in the rules crate and cloned into a scenario.
pub(crate) fn nato_map() -> MapDefinition {
    static MAP: OnceLock<MapDefinition> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map: MapDefinition = serde_json::from_str(include_str!("../../data/natoMap.json"))
            .expect("embedded NATO map data must be valid");
        if map.id.is_empty() {
            map.id = NATO_MAP_ID.to_owned();
        }
        // 2.2.2, 33.1: the land map-edge hex nearest each printed Reinforcement
        // Box. Sectors 1-2 run along the western edge (NATO), 3-5 the eastern (WP).
        map.reinforcement_sectors = [
            (1, "nato", "3534"),
            (2, "nato", "4734"),
            (3, "warsawPact", "4301"),
            (4, "warsawPact", "3301"),
            (5, "warsawPact", "2501"),
        ]
        .into_iter()
        .map(|(number, side, hex)| ReinforcementSector {
            number,
            side_id: SideId(side.to_owned()),
            hex_id: HexId(hex.to_owned()),
        })
        .collect();
        map
    })
    .clone()
}
