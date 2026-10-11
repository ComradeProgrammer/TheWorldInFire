//! NATO Fortified hexes (Extended Buildup, rule 38.4.3.4).
//!
//! Fortified hexes help only NATO units defending against the Warsaw Pact. A
//! hex stays Fortified until a Joint Status Phase finds a Warsaw Pact unit in
//! it, and never regains the status afterwards.

use crate::event::GameEvent;
use crate::model::{HexId, SideId, UnitLocation};
use crate::rules::Rules;

/// The only side whose attacks and Strikes Fortified hexes resist.
const WARSAW_PACT: &str = "warsawPact";
/// The only side Fortified hexes protect.
const NATO: &str = "nato";

impl Rules {
    /// Whether `hex_id` is Fortified against an attack or Strike by `attacking_side`.
    pub(crate) fn fortified_against(&self, attacking_side: &SideId, hex_id: &HexId) -> bool {
        attacking_side.0 == WARSAW_PACT && self.fortified_hexes.contains(hex_id)
    }

    /// Whether `hex_id` is Fortified for units of `defending_side` defending in it.
    pub(crate) fn fortified_for(&self, defending_side: &SideId, hex_id: &HexId) -> bool {
        defending_side.0 == NATO && self.fortified_hexes.contains(hex_id)
    }

    /// Joint Status Phase: Fortified hexes occupied by a Warsaw Pact unit are Conquered.
    pub(crate) fn conquer_fortified_hexes(&mut self, events: &mut Vec<GameEvent>) {
        let occupied = |hex_id: &HexId| {
            self.units.values().any(|unit| {
                unit.definition.side_id.0 == WARSAW_PACT
                    && matches!(&unit.location, UnitLocation::Hex { hex_id: at } if at == hex_id)
            })
        };
        let conquered: Vec<HexId> = self
            .fortified_hexes
            .iter()
            .filter(|hex_id| occupied(hex_id))
            .cloned()
            .collect();
        if conquered.is_empty() {
            return;
        }
        self.fortified_hexes
            .retain(|hex_id| !conquered.contains(hex_id));
        events.push(GameEvent::FortifiedHexesConquered { hex_ids: conquered });
    }
}
