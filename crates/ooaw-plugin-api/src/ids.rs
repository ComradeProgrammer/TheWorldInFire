use serde::{Deserialize, Serialize};

/// Stable identifier for one running or saved game session.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameId(
    /// String representation exchanged with clients and stored in saves.
    pub String,
);

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
