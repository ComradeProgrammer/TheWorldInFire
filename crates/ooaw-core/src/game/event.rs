use serde::{Deserialize, Serialize};

use crate::scenario::PhaseDefinition;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GameEvent {
    PhaseEnded {
        game_turn: u16,
        step: PhaseDefinition,
    },
    PhaseStarted {
        game_turn: u16,
        step: PhaseDefinition,
    },
    GameTurnStarted {
        game_turn: u16,
    },
    GameCompleted {
        game_turn: u16,
    },
}
