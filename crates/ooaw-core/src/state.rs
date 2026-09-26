use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::model::{
    PhaseActor, PhaseDefinition, ScenarioDefinition, ScenarioSummary, UnitId, UnitState,
};

use crate::error::RuleError;

/// Stable identifier for one running or saved game session.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameId(
    /// String representation exchanged with clients and stored in saves.
    pub String,
);

/// High-level lifecycle state of a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameStatus {
    /// Commands may still advance the game.
    InProgress,
    /// The scenario has ended and no further game commands are accepted.
    Completed,
}

/// Current position within a scenario's turn sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnState {
    /// One-based game-turn number.
    pub game_turn: u16,
    /// Zero-based index into the scenario-defined turn sequence.
    pub step_index: usize,
    /// Active phase, or `None` after the game has completed.
    pub current_step: Option<PhaseDefinition>,
}

/// Complete client-facing representation of the authoritative game state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSnapshot {
    /// Version of the serialized snapshot contract.
    pub protocol_version: u16,
    /// Identifier of the game represented by this snapshot.
    pub game_id: GameId,
    /// Monotonic revision used for optimistic concurrency checks.
    pub revision: u64,
    /// Public metadata for the selected scenario.
    pub scenario: ScenarioSummary,
    /// Current game lifecycle status.
    pub status: GameStatus,
    /// Current game turn and phase.
    pub turn: TurnState,
    /// All units that have entered play so far.
    pub units: Vec<UnitState>,
    /// Player decision that must be resolved before automatic play can continue.
    pub pending_decision: Option<PendingDecision>,
}

/// Description of an input currently required from a player.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDecision {
    /// Stable identifier for the kind of decision the client must present.
    pub kind: String,
}

/// Mutable authoritative state owned by the rules engine.
pub struct GameState {
    pub(super) game_id: GameId,
    pub(super) scenario: ScenarioDefinition,
    pub(super) revision: u64,
    pub(super) status: GameStatus,
    pub(super) game_turn: u16,
    pub(super) step_index: usize,
    pub(super) units: BTreeMap<UnitId, UnitState>,
}

impl GameState {
    /// Creates a game at the first step of the supplied scenario.
    pub fn new(game_id: GameId, scenario: ScenarioDefinition) -> Result<Self, RuleError> {
        validate_scenario(&scenario)?;
        Ok(Self {
            game_id,
            scenario,
            revision: 0,
            status: GameStatus::InProgress,
            game_turn: 1,
            step_index: 0,
            units: BTreeMap::new(),
        })
    }

    /// Returns the serializable game state exposed to clients.
    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            protocol_version: 4,
            game_id: self.game_id.clone(),
            revision: self.revision,
            scenario: ScenarioSummary::from(&self.scenario),
            status: self.status,
            turn: TurnState {
                game_turn: self.game_turn,
                step_index: self.step_index,
                current_step: self.current_step().cloned(),
            },
            units: self.units.values().cloned().collect(),
            pending_decision: None,
        }
    }

    pub(super) fn current_step(&self) -> Option<&PhaseDefinition> {
        if self.status == GameStatus::Completed {
            None
        } else {
            self.scenario.turn_sequence.get(self.step_index)
        }
    }
}

fn validate_scenario(scenario: &ScenarioDefinition) -> Result<(), RuleError> {
    if scenario.max_game_turns == 0 {
        return Err(RuleError::new(
            "invalidScenario",
            "A scenario must contain at least one game turn",
        ));
    }
    if scenario.turn_sequence.is_empty() {
        return Err(RuleError::new(
            "invalidScenario",
            "A scenario must contain at least one turn step",
        ));
    }
    for step in &scenario.turn_sequence {
        if let PhaseActor::Side { side_id } = &step.actor {
            if !scenario.sides.iter().any(|side| side.id == *side_id) {
                return Err(RuleError::new(
                    "invalidScenario",
                    format!("Turn step references unknown side: {}", side_id.0),
                ));
            }
        }
    }

    let mut unit_ids = HashSet::new();
    for reinforcement in &scenario.reinforcements {
        let unit = &reinforcement.unit;
        if reinforcement.game_turn == 0 || reinforcement.game_turn > scenario.max_game_turns {
            return Err(RuleError::new(
                "invalidScenario",
                format!(
                    "Unit {} has an invalid reinforcement turn: {}",
                    unit.id().0,
                    reinforcement.game_turn
                ),
            ));
        }
        if !unit_ids.insert(unit.id().clone()) {
            return Err(RuleError::new(
                "invalidScenario",
                format!("Duplicate unit ID: {}", unit.id().0),
            ));
        }
        if !scenario
            .sides
            .iter()
            .any(|side| side.id == unit.definition.side_id)
        {
            return Err(RuleError::new(
                "invalidScenario",
                format!("Unit {} references an unknown side", unit.id().0),
            ));
        }
        if unit.definition.steps.is_empty()
            || unit.strength_step_index >= unit.definition.steps.len()
        {
            return Err(RuleError::new(
                "invalidScenario",
                format!("Unit {} has an invalid strength-step setup", unit.id().0),
            ));
        }
    }
    Ok(())
}
