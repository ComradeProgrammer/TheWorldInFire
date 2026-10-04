use crate::engine::GameEngine;
use crate::event::GameEvent;
use crate::movement::MovementContext;
use std::collections::{HashSet, VecDeque};

use crate::model::{HexId, PhaseActor, Terrain, TrainStatus, UnitLocation, UnitState};

impl GameEngine {
    /// Dispatches initialization and automatic rules for the newly active phase.
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

    /// Provides the Joint Status Phase hook, which currently has no automatic effects.
    fn resolve_joint_status(&mut self, _events: &mut Vec<GameEvent>) {}

    /// Resets air points, withdraws scheduled units, and adds this turn's reinforcements.
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

        // Place one at a time so each arrival sees the units placed before it.
        let mut arriving_units = arriving_units;
        for unit in &mut arriving_units {
            unit.location = self.arrival_location(unit);
            // Rail reinforcements arrive Entrained while rail capacity allows (13.6).
            if unit.train_status == Some(TrainStatus::Entrained)
                && self
                    .validate_rail_capacity(&unit.definition.side_id, unit)
                    .is_err()
            {
                unit.train_status = Some(TrainStatus::Entraining);
            }
            self.units.insert(unit.id().clone(), unit.clone());
        }

        if !arriving_units.is_empty() {
            events.push(GameEvent::ReinforcementsArrived {
                game_turn: self.game_turn,
                units: arriving_units,
            });
        }
    }

    /// Supplies off-map reserve units and emits the acting side's recorded supply checks.
    fn resolve_pre_battle(&mut self, events: &mut Vec<GameEvent>) {
        let Some(side_id) = self.current_step().and_then(|step| match &step.actor {
            PhaseActor::Side { side_id } => Some(side_id.clone()),
            PhaseActor::All => None,
        }) else {
            return;
        };

        // Simplified supply (see `supply.rs`): units in the Strategic Reserve are
        // supplied; units on the map trace to a friendly Free City or a supplied HQ.
        let checked_units = self.update_supply(&side_id, events);

        events.push(GameEvent::PreBattleSupplyChecked {
            game_turn: self.game_turn,
            side_id,
            units: checked_units,
        });
    }

    /// Starts the acting side's battle plan and completes eligible train loading.
    fn resolve_battle_planning(&mut self, events: &mut Vec<GameEvent>) {
        self.start_battle_plan(events);
    }

    /// Starts an empty air mission plan for the acting side's Offensive Strike Phase.
    fn resolve_offensive_strike(&mut self, _events: &mut Vec<GameEvent>) {
        self.start_strike_plan();
    }

    /// Starts the acting side's combat records and per-phase restrictions.
    fn resolve_combat(&mut self, _events: &mut Vec<GameEvent>) {
        self.start_combat();
    }

    /// Starts reserve movement with the acting side's eligible marked units.
    fn resolve_reserve(&mut self, _events: &mut Vec<GameEvent>) {
        self.start_reserve();
    }

    /// Removes the acting side's Suppressed markers and emits the resulting changes.
    fn resolve_post_battle(&mut self, events: &mut Vec<GameEvent>) {
        self.remove_suppression(events);
    }
}

/// How far a reinforcement may be displaced from a full entry hex.
const ARRIVAL_SPREAD: u16 = 3;

impl GameEngine {
    /// Where an arriving reinforcement is placed (house rule replacing the
    /// Reinforcement Boxes). Turn-one units are the opening setup and are
    /// placed as written. A later unit due in an enemy-held hex (enemy units or
    /// an enemy-controlled city) goes to the Strategic Reserve instead; a unit
    /// due in a full hex takes the nearest hex with room, up to three away.
    fn arrival_location(&self, unit: &UnitState) -> UnitLocation {
        let UnitLocation::Hex { hex_id } = &unit.location else {
            return unit.location.clone();
        };
        // The opening deployment is the scenario's setup: place it as written.
        if self.game_turn == 1 {
            return unit.location.clone();
        }
        let side_id = &unit.definition.side_id;
        let enemy_held = |hex: &HexId| {
            self.units.values().any(|other| {
                other.definition.side_id != *side_id
                    && matches!(&other.location, UnitLocation::Hex { hex_id } if hex_id == hex)
            }) || self
                .city_control
                .get(hex)
                .is_some_and(|controller| controller != side_id)
        };
        let land = |hex: &HexId| {
            self.scenario
                .map
                .hexes
                .iter()
                .any(|candidate| candidate.id == *hex && candidate.terrain != Terrain::Sea)
        };
        if enemy_held(hex_id) || !land(hex_id) {
            return UnitLocation::StrategicReserve;
        }
        let context = MovementContext::new(self, side_id);
        let mut seen = HashSet::from([hex_id.0.clone()]);
        let mut queue = VecDeque::from([(hex_id.0.clone(), 0_u16)]);
        while let Some((current, distance)) = queue.pop_front() {
            let candidate = HexId(current.clone());
            if !enemy_held(&candidate)
                && land(&candidate)
                && self.validate_destination_stacking(unit, &candidate).is_ok()
            {
                return UnitLocation::Hex { hex_id: candidate };
            }
            if distance == ARRIVAL_SPREAD {
                continue;
            }
            for next in context.neighbors(&current) {
                if seen.insert(next.to_owned()) {
                    queue.push_back((next.to_owned(), distance + 1));
                }
            }
        }
        UnitLocation::StrategicReserve
    }
}
