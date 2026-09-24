use crate::scenario::PhaseExecution;

use super::{CommandOutcome, GameCommand, GameEvent, GameState, GameStatus, RuleError};

impl GameState {
    /// Executes a command against the authoritative state and returns its outcome.
    pub fn execute(&mut self, command: GameCommand) -> Result<CommandOutcome, RuleError> {
        if self.status == GameStatus::Completed {
            return Err(RuleError::game_complete());
        }

        let events = match command {
            GameCommand::EndPhase => self.end_phase(),
        };
        self.revision += 1;
        Ok(CommandOutcome {
            revision: self.revision,
            events,
            snapshot: self.snapshot(),
        })
    }

    fn end_phase(&mut self) -> Vec<GameEvent> {
        let mut events = Vec::new();
        if let Some(step) = self.current_step().cloned() {
            events.push(GameEvent::PhaseEnded {
                game_turn: self.game_turn,
                step,
            });
        }

        self.move_to_next_step(&mut events);
        while self.status == GameStatus::InProgress
            && self
                .current_step()
                .is_some_and(|step| matches!(step.execution, PhaseExecution::Automatic))
        {
            let step = self.current_step().expect("checked above").clone();
            events.push(GameEvent::PhaseEnded {
                game_turn: self.game_turn,
                step,
            });
            self.move_to_next_step(&mut events);
        }
        events
    }

    fn move_to_next_step(&mut self, events: &mut Vec<GameEvent>) {
        self.step_index += 1;
        if self.step_index >= self.scenario.turn_sequence.len() {
            if self.game_turn >= self.scenario.max_game_turns {
                self.status = GameStatus::Completed;
                self.step_index = self.scenario.turn_sequence.len();
                events.push(GameEvent::GameCompleted {
                    game_turn: self.game_turn,
                });
                return;
            }
            self.game_turn += 1;
            self.step_index = 0;
            events.push(GameEvent::GameTurnStarted {
                game_turn: self.game_turn,
            });
        }

        if let Some(step) = self.current_step().cloned() {
            events.push(GameEvent::PhaseStarted {
                game_turn: self.game_turn,
                step,
            });
        }
    }
}
