use serde::{Deserialize, Serialize};

use crate::scenario::{PhaseActor, PhaseDefinition, ScenarioDefinition, ScenarioSummary};

use super::RuleError;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameStatus {
    InProgress,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnState {
    pub game_turn: u16,
    pub step_index: usize,
    pub current_step: Option<PhaseDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSnapshot {
    pub protocol_version: u16,
    pub game_id: GameId,
    pub revision: u64,
    pub scenario: ScenarioSummary,
    pub status: GameStatus,
    pub turn: TurnState,
    pub pending_decision: Option<PendingDecision>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDecision {
    pub kind: String,
}

pub struct GameState {
    pub(super) game_id: GameId,
    pub(super) scenario: ScenarioDefinition,
    pub(super) revision: u64,
    pub(super) status: GameStatus,
    pub(super) game_turn: u16,
    pub(super) step_index: usize,
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
        })
    }

    /// Returns the serializable game state exposed to clients.
    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            protocol_version: 1,
            game_id: self.game_id.clone(),
            revision: self.revision,
            scenario: ScenarioSummary::from(&self.scenario),
            status: self.status,
            turn: TurnState {
                game_turn: self.game_turn,
                step_index: self.step_index,
                current_step: self.current_step().cloned(),
            },
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
    Ok(())
}
