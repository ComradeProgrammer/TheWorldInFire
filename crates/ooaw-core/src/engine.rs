use crate::command::{CommandOutcome, GameCommand};
use crate::error::RuleError;
use crate::event::GameEvent;
use crate::model::PhaseExecution;
use crate::state::{GameState, GameStatus};

impl GameState {
    /// Executes a command against the authoritative state and returns its outcome.
    pub fn execute(&mut self, command: GameCommand) -> Result<CommandOutcome, RuleError> {
        if self.status == GameStatus::Completed {
            return Err(RuleError::game_complete());
        }

        let events = match command {
            GameCommand::EndPhase => self.end_phase(),
            GameCommand::SetResupplyTarget { unit_id } => self.set_resupply_target(unit_id),
            GameCommand::SetAttackTarget { hex_id, selected } => {
                self.set_attack_target(hex_id, selected)
            }
            GameCommand::MoveUnit {
                unit_id,
                destination,
                mode,
            } => self.move_unit(unit_id, destination, mode),
            GameCommand::UndoUnitMovement { unit_id } => self.undo_unit_movement(unit_id),
            GameCommand::EntrainUnit { unit_id } => self.entrain_unit(unit_id),
            GameCommand::DetrainUnit { unit_id } => self.detrain_unit(unit_id),
            GameCommand::UndoDetrainUnit { unit_id } => self.undo_detrain_unit(unit_id),
            GameCommand::PlanAirStrike {
                hex_id,
                unit_ids,
                air_point,
            } => self.plan_air_strike(hex_id, unit_ids, air_point),
            GameCommand::PlanAirInterdiction { hex_id, air_point } => {
                self.plan_air_interdiction(hex_id, air_point)
            }
            GameCommand::CancelAirMission { mission_id } => self.cancel_air_mission(mission_id),
            GameCommand::ResolveAirStrikes => self.resolve_air_strikes(),
        }?;
        self.revision += 1;
        Ok(CommandOutcome {
            revision: self.revision,
            events,
            snapshot: self.snapshot(),
        })
    }

    fn end_phase(&mut self) -> Result<Vec<GameEvent>, RuleError> {
        let mut events = Vec::new();
        if let Some(step) = self.current_step().cloned() {
            match step.phase_id.0.as_str() {
                "battlePlanning" => self.finish_battle_plan(&mut events),
                "offensiveStrike" => self.finish_offensive_strike(&mut events),
                "reserve" => self.finish_reserve(&mut events),
                _ => {}
            }
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
        Ok(events)
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
            self.on_phase_started(events);
        }
    }
}
