use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{FormationId, HexId, NationId, SideId, UnitId, UnitTraitId, UnitTypeId};

/// Current board or off-map position of a unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UnitLocation {
    /// The unit occupies a specific map hex.
    Hex {
        /// Coordinate of the occupied map hex.
        hex_id: HexId,
    },
    /// The unit is available in its side's off-map strategic reserve.
    StrategicReserve,
}

/// A unit as the kernel stores it: identity, step track, and location, plus
/// rules-owned markers.
///
/// The kernel interprets only the fields it needs to keep state consistent.
/// Printed step values and every other rules-specific property (supply, train
/// status, disruption, or anything a plugin adds) are opaque JSON. Markers are
/// flattened into the unit object, so a plugin's own typed unit struct with the
/// same field names reads and writes this shape directly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unit {
    /// Stable identifier for this unit.
    pub id: UnitId,
    /// Full human-readable unit name.
    pub name: String,
    /// Side that owns and controls the unit.
    pub side_id: SideId,
    /// National force to which the unit belongs.
    pub nation_id: NationId,
    /// Military function used to apply unit-type rules.
    pub unit_type_id: UnitTypeId,
    /// Parent formation, when any.
    pub formation_id: Option<FormationId>,
    /// Additional rules-relevant characteristics.
    pub traits: Vec<UnitTraitId>,
    /// Printed values of each strength step, full strength first; the rules
    /// plugin defines their fields.
    pub steps: Vec<Map<String, Value>>,
    /// Index of the currently active entry in [`Self::steps`].
    pub strength_step_index: usize,
    /// Current map or off-map position.
    pub location: UnitLocation,
    /// Rules-owned unit state keyed by field name, such as `supply`.
    #[serde(flatten)]
    pub markers: BTreeMap<String, Value>,
}

/// Unit fields the kernel defines; every other top-level field is a marker.
pub const KERNEL_UNIT_FIELDS: [&str; 10] = [
    "id",
    "name",
    "sideId",
    "nationId",
    "unitTypeId",
    "formationId",
    "traits",
    "steps",
    "strengthStepIndex",
    "location",
];
