use serde::{Deserialize, Serialize};

use super::{Disruption, SideId, TrainStatus};

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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

/// Whether a unit currently has access to one kind of supply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SupplyStatus {
    /// The unit can use the capabilities governed by this supply type.
    Supplied,
    /// The unit is subject to the restrictions of this supply type.
    OutOfSupply,
}

/// Supply state carried by a unit in the authoritative game state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitSupplyState {
    /// HQ supply status; present only for headquarters units.
    pub headquarters: Option<SupplyStatus>,
    /// Movement supply status; present only for combat units.
    pub movement: Option<SupplyStatus>,
    /// Combat supply status; present only for combat units.
    pub combat: Option<SupplyStatus>,
}

impl UnitSupplyState {
    /// Creates the initial fully supplied state of a headquarters unit.
    pub fn supplied_headquarters() -> Self {
        Self {
            headquarters: Some(SupplyStatus::Supplied),
            movement: None,
            combat: None,
        }
    }

    /// Creates the initial fully supplied state of a combat unit.
    pub fn supplied_combat_unit() -> Self {
        Self {
            headquarters: None,
            movement: Some(SupplyStatus::Supplied),
            combat: Some(SupplyStatus::Supplied),
        }
    }
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
    /// Current HQ, movement, and combat supply statuses applicable to this unit.
    pub supply: UnitSupplyState,
    /// Rail-loading state that persists between player turns.
    pub train_status: Option<TrainStatus>,
    /// Disrupted or Suppressed marker from an enemy Strike or battle (25.6.4).
    #[serde(default)]
    pub disruption: Option<Disruption>,
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

    /// Marks every applicable supply type as supplied.
    pub(crate) fn mark_fully_supplied(&mut self) {
        if let Some(status) = &mut self.supply.headquarters {
            *status = SupplyStatus::Supplied;
        }
        if let Some(status) = &mut self.supply.movement {
            *status = SupplyStatus::Supplied;
        }
        if let Some(status) = &mut self.supply.combat {
            *status = SupplyStatus::Supplied;
        }
    }

    /// Number of surviving physical steps represented by this unit.
    pub(crate) fn step_count(&self) -> u16 {
        self.definition
            .steps
            .len()
            .saturating_sub(self.strength_step_index) as u16
    }

    /// Whether the unit is a headquarters rather than a maneuver unit.
    pub(crate) fn is_headquarters(&self) -> bool {
        self.supply.headquarters.is_some()
    }

    /// Whether the unit carries a scenario or rules trait.
    pub(crate) fn has_trait(&self, name: &str) -> bool {
        self.definition.traits.iter().any(|item| item.0 == name)
    }

    /// Whether the unit may continue through enemy zones of control.
    pub(crate) fn is_hard(&self) -> bool {
        self.definition.traits.iter().any(|item| item.0 == "hard")
    }

    /// Whether the unit is eligible to be carried by an Airlift Command.
    pub(crate) fn is_air_transportable(&self) -> bool {
        self.definition
            .traits
            .iter()
            .any(|item| item.0 == "airTransportable")
    }
}
