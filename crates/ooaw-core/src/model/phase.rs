use serde::{Deserialize, Serialize};

use super::SideId;

/// Stable identifier for a reusable phase type such as `combat`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhaseId(
    /// String representation used by scenario data and clients.
    pub String,
);

/// Stable identifier for one concrete step in a scenario turn sequence.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepId(
    /// String representation that uniquely identifies the turn step.
    pub String,
);

/// Side or group of sides responsible for acting during a phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PhaseActor {
    /// Every side participates in the phase.
    All,
    /// One scenario-defined side acts during the phase.
    Side {
        /// Identifier of the acting side.
        side_id: SideId,
    },
}

/// Determines whether phase progression requires client input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhaseExecution {
    /// The client must submit a command to finish the phase.
    Interactive,
    /// Rules at runtime determine whether client input is required.
    Conditional,
    /// The engine resolves and advances the phase without client input.
    Automatic,
}

/// One ordered step in a scenario's turn sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseDefinition {
    /// Identifier unique within the scenario turn sequence.
    pub id: StepId,
    /// Reusable phase type handled by the rules engine.
    pub phase_id: PhaseId,
    /// Side or sides responsible for the phase.
    pub actor: PhaseActor,
    /// Input behavior used when the engine reaches the phase.
    pub execution: PhaseExecution,
}
