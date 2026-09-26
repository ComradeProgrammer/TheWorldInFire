use crate::event::GameEvent;
use crate::state::GameState;

impl GameState {
    pub(super) fn on_phase_started(&mut self, events: &mut Vec<GameEvent>) {
        let Some(phase_id) = self.current_step().map(|step| step.phase_id.0.clone()) else {
            return;
        };

        match phase_id.as_str() {
            "jointStatus" => self.resolve_joint_status(events),
            "jointReinforcement" => self.resolve_joint_reinforcement(events),
            "preBattle" => self.resolve_pre_battle(events),
            "battlePlanning" => self.resolve_battle_planning(events),
            "offensiveStrike" => self.resolve_offensive_strike(events),
            "combat" => self.resolve_combat(events),
            "reserve" => self.resolve_reserve(events),
            "postBattle" => self.resolve_post_battle(events),
            _ => {}
        }
    }

    fn resolve_joint_status(&mut self, _events: &mut Vec<GameEvent>) {}

    fn resolve_joint_reinforcement(&mut self, events: &mut Vec<GameEvent>) {
        let arriving_units: Vec<_> = self
            .scenario
            .reinforcements
            .iter()
            .filter(|reinforcement| reinforcement.game_turn == self.game_turn)
            .map(|reinforcement| reinforcement.unit.clone())
            .filter(|unit| !self.units.contains_key(unit.id()))
            .collect();

        for unit in &arriving_units {
            self.units.insert(unit.id().clone(), unit.clone());
        }

        if !arriving_units.is_empty() {
            events.push(GameEvent::ReinforcementsArrived {
                game_turn: self.game_turn,
                units: arriving_units,
            });
        }
    }

    fn resolve_pre_battle(&mut self, _events: &mut Vec<GameEvent>) {}

    fn resolve_battle_planning(&mut self, _events: &mut Vec<GameEvent>) {}

    fn resolve_offensive_strike(&mut self, _events: &mut Vec<GameEvent>) {}

    fn resolve_combat(&mut self, _events: &mut Vec<GameEvent>) {}

    fn resolve_reserve(&mut self, _events: &mut Vec<GameEvent>) {}

    fn resolve_post_battle(&mut self, _events: &mut Vec<GameEvent>) {}
}
