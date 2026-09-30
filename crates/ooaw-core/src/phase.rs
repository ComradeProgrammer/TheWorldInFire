use crate::engine::GameEngine;
use crate::event::{GameEvent, UnitSupplyCheck};
use crate::model::{PhaseActor, UnitLocation};

impl GameEngine {
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
        self.reset_air_points(events);
        let withdrawn: Vec<_> = self
            .scenario
            .withdrawals
            .iter()
            .filter(|withdrawal| withdrawal.game_turn == self.game_turn)
            .map(|withdrawal| withdrawal.unit_id.clone())
            .filter(|unit_id| self.units.contains_key(unit_id))
            .collect();
        for unit_id in withdrawn {
            self.units.remove(&unit_id);
            events.push(GameEvent::UnitWithdrawn {
                game_turn: self.game_turn,
                unit_id,
            });
        }

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

    fn resolve_pre_battle(&mut self, events: &mut Vec<GameEvent>) {
        let Some(side_id) = self.current_step().and_then(|step| match &step.actor {
            PhaseActor::Side { side_id } => Some(side_id.clone()),
            PhaseActor::All => None,
        }) else {
            return;
        };

        let mut checked_units = Vec::new();
        for unit in self
            .units
            .values_mut()
            .filter(|unit| unit.definition.side_id == side_id)
        {
            // Reinforcements and units in the Strategic Reserve are always supplied.
            // On-map LOS calculation will replace the retained on-map state once the
            // authoritative map-control and HQ-range model is available.
            if matches!(unit.location, UnitLocation::StrategicReserve) {
                unit.mark_fully_supplied();
            }
            checked_units.push(UnitSupplyCheck {
                unit_id: unit.id().clone(),
                supply: unit.supply.clone(),
            });
        }

        events.push(GameEvent::PreBattleSupplyChecked {
            game_turn: self.game_turn,
            side_id,
            units: checked_units,
        });
    }

    fn resolve_battle_planning(&mut self, events: &mut Vec<GameEvent>) {
        self.start_battle_plan(events);
    }

    fn resolve_offensive_strike(&mut self, _events: &mut Vec<GameEvent>) {
        self.start_strike_plan();
    }

    fn resolve_combat(&mut self, _events: &mut Vec<GameEvent>) {
        self.start_combat();
    }

    fn resolve_reserve(&mut self, _events: &mut Vec<GameEvent>) {
        self.start_reserve();
    }

    fn resolve_post_battle(&mut self, events: &mut Vec<GameEvent>) {
        self.remove_suppression(events);
    }
}
