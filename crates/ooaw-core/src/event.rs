use serde::{Deserialize, Serialize};

use crate::model::{PhaseDefinition, UnitState};

/// A fact emitted after the authoritative game state changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GameEvent {
    /// The indicated phase has finished.
    PhaseEnded {
        /// Game turn on which the phase ended.
        game_turn: u16,
        /// Definition of the phase that ended.
        step: PhaseDefinition,
    },
    /// The indicated phase has become current.
    PhaseStarted {
        /// Game turn on which the phase started.
        game_turn: u16,
        /// Definition of the phase that started.
        step: PhaseDefinition,
    },
    /// A new game turn has begun.
    GameTurnStarted {
        /// Newly active game turn.
        game_turn: u16,
    },
    /// Scenario-scheduled units have entered authoritative state.
    ReinforcementsArrived {
        /// Game turn on which the units arrived.
        game_turn: u16,
        /// Complete states of the newly arrived units.
        units: Vec<UnitState>,
    },
    /// The scenario has reached its final turn and phase.
    GameCompleted {
        /// Game turn on which play ended.
        game_turn: u16,
    },
}
