use serde::{Deserialize, Serialize};

use crate::event::GameEvent;
use crate::state::GameSnapshot;

/// A player or client request that may change the authoritative game state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GameCommand {
    /// Finish the current interactive phase and advance the turn sequence.
    EndPhase,
}

/// The events and current snapshot produced by an accepted game command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandOutcome {
    /// The authoritative revision after the command has been applied.
    pub revision: u64,
    /// Ordered domain events emitted while processing the command.
    pub events: Vec<GameEvent>,
    /// The complete authoritative state after command processing.
    pub snapshot: GameSnapshot,
}
