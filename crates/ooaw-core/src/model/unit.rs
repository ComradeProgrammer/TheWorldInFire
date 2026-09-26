use serde::{Deserialize, Serialize};

use super::SideId;

/// Stable identifier for a physical military unit in a scenario.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnitId(
    /// String representation used by commands, events, and saves.
    pub String,
);

/// Stable identifier for a unit's nationality.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NationId(
    /// String representation used by scenario and presentation data.
    pub String,
);

/// Stable identifier for the military function represented by a unit.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnitTypeId(
    /// String representation of the unit type.
    pub String,
);

/// Stable identifier for an additional rules-relevant unit characteristic.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnitTraitId(
    /// String representation of the unit trait.
    pub String,
);

/// Stable identifier for a parent military formation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FormationId(
    /// String representation of the formation identifier.
    pub String,
);

/// Stable board coordinate identifying one map hex.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HexId(
    /// Scenario map's external hex label, such as `2806`.
    pub String,
);

/// Printed combat and movement values for one strength step of a unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitStepDefinition {
    /// Leftmost numeric counter value, currently exposed as attack strength.
    pub attack: u16,
    /// Numeric defense value; defense classification is not yet modeled.
    pub defense: u16,
    /// Movement-point allowance currently represented by this step.
    pub movement: u16,
}

/// Scenario-independent identity and capabilities of a military unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitDefinition {
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
    /// Parent formation used for organization and support rules, when any.
    pub formation_id: Option<FormationId>,
    /// Additional rules-relevant characteristics not captured by unit type.
    pub traits: Vec<UnitTraitId>,
    /// Strength steps ordered from full strength to most reduced.
    pub steps: Vec<UnitStepDefinition>,
}

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

/// Mutable in-game state of a unit together with its immutable definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitState {
    /// Identity, ownership, capabilities, and printed strength steps.
    #[serde(flatten)]
    pub definition: UnitDefinition,
    /// Index of the currently active entry in [`UnitDefinition::steps`].
    pub strength_step_index: usize,
    /// Current map or off-map position.
    pub location: UnitLocation,
}

impl UnitState {
    /// Returns the stable identifier used by commands, events, and snapshots.
    pub fn id(&self) -> &UnitId {
        &self.definition.id
    }

    /// Returns the combat values for the unit's current strength step.
    pub fn current_step(&self) -> Option<&UnitStepDefinition> {
        self.definition.steps.get(self.strength_step_index)
    }
}
