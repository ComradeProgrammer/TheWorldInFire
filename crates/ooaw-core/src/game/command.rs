use serde::{Deserialize, Serialize};

use super::{GameEvent, GameSnapshot};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GameCommand {
    EndPhase,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandOutcome {
    pub revision: u64,
    pub events: Vec<GameEvent>,
    pub snapshot: GameSnapshot,
}
