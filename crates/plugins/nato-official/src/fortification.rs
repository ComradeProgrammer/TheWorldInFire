//! NATO Fortified hexes (Extended Buildup, rule 38.4.3.4).
//!
//! Fortified hexes help only NATO units defending against the Warsaw Pact. By
//! house rule they stay Fortified for the whole game: Warsaw Pact occupation
//! never Conquers them.

use crate::model::{HexId, SideId};
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
}
