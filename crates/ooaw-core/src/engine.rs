use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::command::{CommandOutcome, GameCommand};
use crate::dice::Dice;
use crate::error::RuleError;
use crate::event::GameEvent;
use crate::model::{
    AirInterdictionZone, AirPoints, BattlePlan, CityControlState, CombatState, HexId,
    MapDefinition, PhaseActor, PhaseDefinition, PhaseExecution, ReserveState, ScenarioDefinition,
    ScenarioSummary, SideId, StrikePlan, UnitId, UnitState,
};

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
    /// Plan currently being assembled or carried into later combat phases.
    pub battle_plan: Option<BattlePlan>,
    /// Control of every city hex on the map.
    pub cities: Vec<CityControlState>,
    /// Air Points held by each side.
    pub air_points: Vec<AirPoints>,
    /// Air missions of the current Offensive Strike Phase.
    pub strike_plan: Option<StrikePlan>,
    /// Active Air Interdiction Zones.
    pub air_interdiction_zones: Vec<AirInterdictionZone>,
    /// Hexes holding a Breakthrough Marker (25.9).
    pub breakthrough_markers: Vec<HexId>,
    /// Units eliminated so far, in order of elimination.
    pub eliminated_unit_ids: Vec<UnitId>,
    /// Battles and restrictions of the current Combat Phase.
    pub combat: Option<CombatState>,
    /// Marked units and their movement in the current Reserve Phase.
    pub reserve: Option<ReserveState>,
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

/// Mutable authoritative state and command processor for a game session.
pub struct GameEngine {
    pub(super) game_id: GameId,
    pub(super) scenario: ScenarioDefinition,
    pub(super) revision: u64,
    pub(super) status: GameStatus,
    pub(super) game_turn: u16,
    pub(super) step_index: usize,
    pub(super) units: BTreeMap<UnitId, UnitState>,
    pub(super) battle_plan: Option<BattlePlan>,
    pub(super) city_control: BTreeMap<HexId, SideId>,
    pub(super) air_points: Vec<AirPoints>,
    pub(super) strike_plan: Option<StrikePlan>,
    pub(super) air_interdiction_zones: Vec<AirInterdictionZone>,
    pub(super) breakthrough_markers: Vec<HexId>,
    pub(super) eliminated_units: Vec<UnitId>,
    pub(super) combat: Option<CombatState>,
    pub(super) reserve: Option<ReserveState>,
    /// Seeded dice; saves must persist this state to replay identically.
    pub(super) dice: Dice,
}

impl GameEngine {
    /// Creates a game at the first step of the supplied scenario.
    pub fn new(game_id: GameId, scenario: ScenarioDefinition) -> Result<Self, RuleError> {
        validate_scenario(&scenario)?;
        let city_control = scenario
            .map
            .hexes
            .iter()
            .filter_map(|hex| {
                hex.city
                    .as_ref()
                    .map(|city| (hex.id.clone(), city.owner.clone()))
            })
            .collect();
        let air_points = scenario
            .sides
            .iter()
            .map(|side| AirPoints {
                side_id: side.id.clone(),
                tactical: 0,
                operational: 0,
                bonus_tactical: scenario
                    .battle_planning_rules
                    .air_power
                    .for_side(&side.id)
                    .bonus_tactical,
            })
            .collect();
        let dice = Dice::from_seed_text(&game_id.0);
        Ok(Self {
            game_id,
            scenario,
            revision: 0,
            status: GameStatus::InProgress,
            game_turn: 1,
            step_index: 0,
            units: BTreeMap::new(),
            battle_plan: None,
            city_control,
            air_points,
            strike_plan: None,
            air_interdiction_zones: Vec::new(),
            breakthrough_markers: Vec::new(),
            eliminated_units: Vec::new(),
            combat: None,
            reserve: None,
            dice,
        })
    }

    /// Returns the serializable game state exposed to clients.
    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            protocol_version: 15,
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
            battle_plan: self.battle_plan.clone(),
            cities: self.city_states(),
            air_points: self.air_points.clone(),
            strike_plan: self.strike_plan.clone(),
            air_interdiction_zones: self.air_interdiction_zones.clone(),
            breakthrough_markers: self.breakthrough_markers.clone(),
            eliminated_unit_ids: self.eliminated_units.clone(),
            combat: self.combat.clone(),
            reserve: self.reserve.clone(),
            // The Attacker must decide on an advance before fighting on (25.8).
            pending_decision: self
                .combat
                .as_ref()
                .and_then(|combat| combat.pending_advance.as_ref())
                .map(|_| PendingDecision {
                    kind: "advanceAfterCombat".to_owned(),
                }),
        }
    }

    pub(super) fn current_step(&self) -> Option<&PhaseDefinition> {
        if self.status == GameStatus::Completed {
            None
        } else {
            self.scenario.turn_sequence.get(self.step_index)
        }
    }

    /// Returns the authoritative map selected by this game's scenario.
    pub fn map(&self) -> &MapDefinition {
        &self.scenario.map
    }

    /// Returns the ordered steps of every game turn in this game's scenario.
    pub fn turn_sequence(&self) -> &[PhaseDefinition] {
        &self.scenario.turn_sequence
    }

    /// Executes a command against the authoritative state and returns its outcome.
    pub fn execute(&mut self, command: GameCommand) -> Result<CommandOutcome, RuleError> {
        if self.status == GameStatus::Completed {
            return Err(RuleError::game_complete());
        }

        let events = match command {
            GameCommand::EndPhase => self.end_phase(),
            GameCommand::SetResupplyTarget { unit_id, selected } => {
                self.set_resupply_target(unit_id, selected)
            }
            GameCommand::SetAttackTarget { hex_id, selected } => {
                self.set_attack_target(hex_id, selected)
            }
            GameCommand::MoveUnit {
                unit_id,
                destination,
                mode,
            } => self.move_unit(unit_id, destination, mode),
            GameCommand::SetReserve { unit_id, selected } => self.set_reserve(unit_id, selected),
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
            GameCommand::ResolveBattle {
                hex_id,
                unit_ids,
                supporting_hq_id,
            } => self.resolve_battle(hex_id, unit_ids, supporting_hq_id),
            GameCommand::AdvanceAfterCombat { unit_ids } => self.advance_after_combat(unit_ids),
        }?;
        self.revision += 1;
        Ok(CommandOutcome {
            revision: self.revision,
            events,
            snapshot: self.snapshot(),
        })
    }

    /// Finishes the current interactive phase and advances to the next one.
    ///
    /// Phase-specific cleanup runs before `PhaseEnded` is emitted. Automatic
    /// phases are then resolved and skipped until the next interactive phase
    /// is reached or the game ends.
    fn end_phase(&mut self) -> Result<Vec<GameEvent>, RuleError> {
        let mut events = Vec::new();
        if let Some(step) = self.current_step().cloned() {
            // Combat cannot end while the rules still require a player decision.
            if step.phase_id.0 == "combat" {
                self.check_combat_can_end()?;
            }

            // Apply any effects that occur when leaving this phase before
            // announcing that the phase has ended.
            match step.phase_id.0.as_str() {
                "battlePlanning" => self.finish_battle_plan(&mut events),
                "offensiveStrike" => self.finish_offensive_strike(&mut events),
                "combat" => self.finish_combat(&mut events),
                "reserve" => {
                    self.finish_reserve(&mut events);
                    self.remove_reserve_markers(&mut events);
                }
                _ => {}
            }
            events.push(GameEvent::PhaseEnded {
                game_turn: self.game_turn,
                step,
            });
        }

        self.move_to_next_step(&mut events);

        // Automatic phases perform their work in `on_phase_started`; clients
        // never stop on them, so keep advancing until input is required again.
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

    /// Starts the next phase, rolling over to a new turn or completing the game
    /// when the current turn has no remaining phases.
    fn move_to_next_step(&mut self, events: &mut Vec<GameEvent>) {
        self.step_index += 1;

        // Passing the end of the turn sequence either starts a new turn or,
        // after the scenario's final turn, completes the game.
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

        // Starting a phase may immediately apply automatic rules and append
        // more events, such as reinforcements arriving or supply being checked.
        if let Some(step) = self.current_step().cloned() {
            events.push(GameEvent::PhaseStarted {
                game_turn: self.game_turn,
                step,
            });
            self.on_phase_started(events);
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

    if scenario.map.hexes.is_empty() {
        return Err(RuleError::new(
            "invalidScenario",
            "A scenario map must contain at least one hex",
        ));
    }

    let mut map_hex_ids = HashSet::new();
    for hex in &scenario.map.hexes {
        if !map_hex_ids.insert(hex.id.clone()) {
            return Err(RuleError::new(
                "invalidScenario",
                format!("Duplicate map hex ID: {}", hex.id.0),
            ));
        }
    }
    for hex in &scenario.map.hexes {
        if let Some(city) = &hex.city {
            if !scenario.sides.iter().any(|side| side.id == city.owner) {
                return Err(RuleError::new(
                    "invalidScenario",
                    format!("City in hex {} has an unknown owner", hex.id.0),
                ));
            }
        }
    }
    for hexside in &scenario.map.hexsides {
        if !map_hex_ids.contains(&hexside.a) || !map_hex_ids.contains(&hexside.b) {
            return Err(RuleError::new(
                "invalidScenario",
                format!(
                    "Map hexside references an unknown hex: {}-{}",
                    hexside.a.0, hexside.b.0
                ),
            ));
        }
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
        if let crate::model::UnitLocation::Hex { hex_id } = &unit.location {
            if !map_hex_ids.contains(hex_id) {
                return Err(RuleError::new(
                    "invalidScenario",
                    format!("Unit {} references an unknown map hex", unit.id().0),
                ));
            }
        }
    }
    Ok(())
}
