use serde::{Deserialize, Serialize};

/// Stable identifier for a playable side in a scenario.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SideId(
    /// Scenario-independent string representation of the side identifier.
    pub String,
);

/// Human-readable metadata for a playable side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SideDefinition {
    /// Stable identifier referenced by phases and units.
    pub id: SideId,
    /// Display name presented to players.
    pub name: String,
}
