use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::dice::Dice;
use crate::model::{
    AirInterdictionZone, AirPoints, BattlePlan, CityControlState, HexId, MapDefinition, PhaseActor,
    PhaseDefinition, ScenarioDefinition, ScenarioSummary, SideId, StrikePlan, UnitId, UnitState,
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
    pub(super) battle_plan: Option<BattlePlan>,
    pub(super) city_control: BTreeMap<HexId, SideId>,
    pub(super) air_points: Vec<AirPoints>,
    pub(super) strike_plan: Option<StrikePlan>,
    pub(super) air_interdiction_zones: Vec<AirInterdictionZone>,
    pub(super) breakthrough_markers: Vec<HexId>,
    pub(super) eliminated_units: Vec<UnitId>,
    /// Seeded dice; saves must persist this state to replay identically.
    pub(super) dice: Dice,
}

impl GameState {
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
            dice,
        })
    }

    /// Returns the serializable game state exposed to clients.
    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            protocol_version: 11,
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

    /// Returns the authoritative map selected by this game's scenario.
    pub fn map(&self) -> &MapDefinition {
        &self.scenario.map
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
