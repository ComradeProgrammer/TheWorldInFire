use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};

use ooaw_plugin_api::protocol::{Change, ScenarioSetup};
use ooaw_plugin_api::serde_json::{self, Value};
use ooaw_plugin_api::{
    GameState, GameStatus, PendingDecision, ScenarioSummary, TurnPosition, Unit,
};
use ooaw_plugin_sdk::{host, mirror};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::command::GameCommand;
use crate::dice::Dice;
use crate::error::RuleError;
use crate::event::GameEvent;
use crate::model::{
    AirInterdictionZone, AirPoints, BattlePlan, BreakthroughMarker, CombatState, HexId, PhaseActor,
    PhaseDefinition, ReserveState, ScenarioDefinition, SideId, StrikePlan, UnitId, UnitState,
};

/// The NATO rules' own top-level state entries, kept by the kernel under these
/// names and shown to clients as snapshot fields.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RulesState {
    /// Each side's battle plan for the current or most recent turn, carried
    /// from its Battle Planning Phase into its later phases.
    pub battle_plans: Vec<BattlePlan>,
    /// Air Points held by each side.
    pub air_points: Vec<AirPoints>,
    /// Air missions of the current Offensive Strike Phase.
    pub strike_plan: Option<StrikePlan>,
    /// Active Air Interdiction Zones.
    pub air_interdiction_zones: Vec<AirInterdictionZone>,
    /// Breakthrough Markers (25.9) and the sides that placed them.
    pub breakthrough_markers: Vec<BreakthroughMarker>,
    /// Units eliminated so far, in order of elimination.
    pub eliminated_unit_ids: Vec<UnitId>,
    /// Battles and restrictions of the current Combat Phase.
    pub combat: Option<CombatState>,
    /// Marked units and their movement in the current Reserve Phase.
    pub reserve: Option<ReserveState>,
}

/// The kernel state as the NATO rules read it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncState {
    units: Vec<UnitState>,
    city_control: BTreeMap<HexId, SideId>,
    rules: RulesState,
}

/// The mirror as last reported to the kernel.
#[derive(Default)]
struct Baseline {
    units: BTreeMap<UnitId, UnitState>,
    city_control: BTreeMap<HexId, SideId>,
    pending_decision: Option<PendingDecision>,
    rules: BTreeMap<String, Value>,
}

/// Encodes a value for the kernel.
pub(crate) fn encode<T: Serialize>(value: &T) -> Result<Value, RuleError> {
    serde_json::to_value(value).map_err(|error| RuleError::protocol(error.to_string()))
}

/// Decodes a kernel message.
pub(crate) fn decode<T: DeserializeOwned>(value: Value) -> Result<T, RuleError> {
    serde_json::from_value(value).map_err(|error| RuleError::new("invalidInput", error.to_string()))
}

/// The NATO rules serving one game: scenario content plus a mirror of the
/// kernel's state that the rule modules read and change directly.
pub struct Rules {
    /// Selected scenario content, map, rules, and scheduled unit arrivals and withdrawals.
    pub(super) scenario: ScenarioDefinition,
    /// Whether the game still accepts commands or has completed its final turn.
    pub(super) status: GameStatus,
    /// One-based game-turn number within the scenario.
    pub(super) game_turn: u16,
    /// Zero-based position in the scenario's repeated turn sequence.
    pub(super) step_index: usize,
    /// Units currently in play, indexed and iterated by stable unit identifier.
    pub(super) units: BTreeMap<UnitId, UnitState>,
    /// Each side's planning selections and movement history, carried into its later phases.
    pub(super) battle_plans: Vec<BattlePlan>,
    /// Current controlling side for each city hex, initially its scenario owner.
    pub(super) city_control: BTreeMap<HexId, SideId>,
    /// Remaining recurring and one-time Air Point pools for each side.
    pub(super) air_points: Vec<AirPoints>,
    /// Missions and resolution state for the current Offensive Strike Phase.
    pub(super) strike_plan: Option<StrikePlan>,
    /// Active interdiction centers that restrict enemy movement in nearby hexes.
    pub(super) air_interdiction_zones: Vec<AirInterdictionZone>,
    /// Cleared hexes with Breakthrough Markers affecting combat and reserve movement.
    pub(super) breakthrough_markers: Vec<BreakthroughMarker>,
    /// Identifiers of units destroyed so far, in elimination order.
    pub(super) eliminated_units: Vec<UnitId>,
    /// Current battles, attack restrictions, support use, and pending advance choice.
    pub(super) combat: Option<CombatState>,
    /// Marked reserve units and their movement history for the current Reserve Phase.
    pub(super) reserve: Option<ReserveState>,
    /// The kernel's seeded dice.
    pub(super) dice: Dice,
    /// State last reported to the kernel, for computing changes.
    baseline: RefCell<Baseline>,
}

impl Rules {
    /// Prepares the rules for a game of the supplied scenario, at the first step
    /// of turn one with city control and Air Point pools from the scenario.
    ///
    /// # Errors
    ///
    /// Returns `invalidScenario` for invalid turn counts, an empty sequence or
    /// map, duplicate hex or unit IDs, unknown side or hex references, or an
    /// invalid reinforcement turn or strength-step setup.
    pub fn new(scenario: ScenarioDefinition) -> Result<Self, RuleError> {
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
        Ok(Self {
            scenario,
            status: GameStatus::InProgress,
            game_turn: 1,
            step_index: 0,
            units: BTreeMap::new(),
            battle_plans: Vec::new(),
            city_control,
            air_points,
            strike_plan: None,
            air_interdiction_zones: Vec::new(),
            breakthrough_markers: Vec::new(),
            eliminated_units: Vec::new(),
            combat: None,
            reserve: None,
            dice: Dice,
            baseline: RefCell::default(),
        })
    }

    /// Scenario content and opening state for the kernel.
    pub(crate) fn scenario_setup(&self) -> Result<ScenarioSetup, RuleError> {
        let units = self
            .units
            .values()
            .map(|unit| encode(unit).and_then(decode::<Unit>))
            .collect::<Result<_, _>>()?;
        Ok(ScenarioSetup {
            scenario: ScenarioSummary::from(&self.scenario),
            map: self.scenario.map.clone(),
            turn_sequence: self.scenario.turn_sequence.clone(),
            state: GameState {
                units,
                city_control: self.city_control.clone(),
                pending_decision: self.pending_decision(),
                rules: self.rules_entries()?,
            },
        })
    }

    /// Records the turn position the kernel sent with a call.
    pub(crate) fn set_turn(&mut self, turn: TurnPosition) {
        self.game_turn = turn.game_turn;
        self.step_index = turn.step_index;
        self.status = turn.status;
    }

    /// Replaces the mirror with the kernel's state and makes it the baseline.
    pub(crate) fn sync(&mut self, state: Value) -> Result<(), RuleError> {
        let state: SyncState = serde_json::from_value(state)
            .map_err(|error| RuleError::protocol(format!("Malformed state: {error}")))?;
        self.units = state
            .units
            .into_iter()
            .map(|unit| (unit.id().clone(), unit))
            .collect();
        self.city_control = state.city_control;
        let rules = state.rules;
        self.battle_plans = rules.battle_plans;
        self.air_points = rules.air_points;
        self.strike_plan = rules.strike_plan;
        self.air_interdiction_zones = rules.air_interdiction_zones;
        self.breakthrough_markers = rules.breakthrough_markers;
        self.eliminated_units = rules.eliminated_unit_ids;
        self.combat = rules.combat;
        self.reserve = rules.reserve;
        *self.baseline.borrow_mut() = Baseline {
            units: self.units.clone(),
            city_control: self.city_control.clone(),
            pending_decision: self.pending_decision(),
            rules: self.rules_entries()?,
        };
        Ok(())
    }

    /// The rules-state entries as the kernel stores them.
    fn rules_entries(&self) -> Result<BTreeMap<String, Value>, RuleError> {
        let state = RulesState {
            battle_plans: self.battle_plans.clone(),
            air_points: self.air_points.clone(),
            strike_plan: self.strike_plan.clone(),
            air_interdiction_zones: self.air_interdiction_zones.clone(),
            breakthrough_markers: self.breakthrough_markers.clone(),
            eliminated_unit_ids: self.eliminated_units.clone(),
            combat: self.combat.clone(),
            reserve: self.reserve.clone(),
        };
        match encode(&state)? {
            Value::Object(map) => Ok(map.into_iter().collect()),
            _ => Err(RuleError::protocol(
                "Rules state must serialize to an object",
            )),
        }
    }

    /// The decision players must make next: the Attacker's advance (25.8).
    fn pending_decision(&self) -> Option<PendingDecision> {
        self.combat
            .as_ref()
            .and_then(|combat| combat.pending_advance.as_ref())
            .map(|_| PendingDecision {
                kind: "advanceAfterCombat".to_owned(),
            })
    }

    /// Changes since the baseline; the current mirror becomes the new baseline.
    pub(crate) fn take_changes(&self) -> Result<Vec<Change>, RuleError> {
        let mut baseline = self.baseline.borrow_mut();
        let mut changes = mirror::unit_changes(&baseline.units, &self.units)?;
        changes.extend(mirror::city_changes(
            &baseline.city_control,
            &self.city_control,
        ));
        let rules = self.rules_entries()?;
        changes.extend(mirror::rules_changes(&baseline.rules, &rules));
        let pending_decision = self.pending_decision();
        changes.extend(mirror::decision_change(
            &baseline.pending_decision,
            &pending_decision,
        ));
        if !changes.is_empty() {
            *baseline = Baseline {
                units: self.units.clone(),
                city_control: self.city_control.clone(),
                pending_decision,
                rules,
            };
        }
        Ok(changes)
    }

    /// Passes `value` through the other plugins taking part in filter `name`.
    ///
    /// Filters are the official rules' extension points: a plugin listing the
    /// filter in its manifest may adjust the value. Pending changes are sent
    /// first so that it sees the current state.
    pub(crate) fn filter<V: Serialize + DeserializeOwned>(
        &self,
        name: &str,
        input: impl Serialize,
        value: V,
    ) -> Result<V, RuleError> {
        if !host::filter_active(name) {
            return Ok(value);
        }
        let changes = self.take_changes()?;
        let result = host::filter(name, encode(&input)?, encode(&value)?, changes)?;
        serde_json::from_value(result).map_err(|error| {
            RuleError::new(
                "invalidFilterValue",
                format!("Filter {name} returned an invalid value: {error}"),
            )
        })
    }

    /// Whether `side_id` has a Breakthrough Marker in the hex.
    pub(crate) fn has_breakthrough(&self, side_id: &SideId, hex_id: &HexId) -> bool {
        self.breakthrough_markers
            .iter()
            .any(|marker| marker.side_id == *side_id && marker.hex_id == *hex_id)
    }

    /// Places a side's Breakthrough Marker unless one is already there (25.9).
    pub(crate) fn place_breakthrough(
        &mut self,
        side_id: &SideId,
        hex_id: &HexId,
        events: &mut Vec<GameEvent>,
    ) {
        if !self.has_breakthrough(side_id, hex_id) {
            self.breakthrough_markers.push(BreakthroughMarker {
                side_id: side_id.clone(),
                hex_id: hex_id.clone(),
            });
            events.push(GameEvent::BreakthroughMarkerPlaced {
                side_id: side_id.clone(),
                hex_id: hex_id.clone(),
            });
        }
    }

    /// The side acting in the current step, if one side acts alone.
    pub(crate) fn acting_side(&self) -> Option<SideId> {
        match &self.current_step()?.actor {
            PhaseActor::Side { side_id } => Some(side_id.clone()),
            PhaseActor::All => None,
        }
    }

    /// A side's battle plan for the current or most recent turn.
    pub(crate) fn plan_for(&self, side_id: &SideId) -> Option<&BattlePlan> {
        self.battle_plans
            .iter()
            .find(|plan| plan.side_id == *side_id)
    }

    /// Mutably borrows a side's battle plan.
    pub(crate) fn plan_for_mut(&mut self, side_id: &SideId) -> Option<&mut BattlePlan> {
        self.battle_plans
            .iter_mut()
            .find(|plan| plan.side_id == *side_id)
    }

    /// Returns the active scenario step, or none once the game has completed.
    pub(super) fn current_step(&self) -> Option<&PhaseDefinition> {
        if self.status == GameStatus::Completed {
            None
        } else {
            self.scenario.turn_sequence.get(self.step_index)
        }
    }

    /// Validates and executes one player command.
    pub(crate) fn execute(&mut self, command: GameCommand) -> Result<Vec<GameEvent>, RuleError> {
        match command {
            GameCommand::EndPhase => Err(RuleError::new(
                "kernelCommand",
                "Ending a phase is handled by the kernel",
            )),
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
            GameCommand::DebugCheckSupply { side_id } => {
                let mut events = Vec::new();
                self.update_supply(&side_id, &mut events);
                Ok(events)
            }
        }
    }

    /// Cleans up the current phase before the kernel ends it, or rejects
    /// ending it while the rules still require a player decision.
    pub(crate) fn phase_ending(&mut self, phase_id: &str) -> Result<Vec<GameEvent>, RuleError> {
        let mut events = Vec::new();
        match phase_id {
            "battlePlanning" => self.finish_battle_plan(&mut events),
            "offensiveStrike" => self.finish_offensive_strike(&mut events)?,
            "combat" => {
                self.check_combat_can_end()?;
                self.finish_combat(&mut events);
            }
            "reserve" => {
                self.finish_reserve(&mut events);
                self.remove_reserve_markers(&mut events);
            }
            _ => {}
        }
        Ok(events)
    }

    /// Hexes in the zones of control of `side_id`'s enemies.
    pub(crate) fn enemy_zoc_hexes(&self, side_id: &SideId) -> Vec<HexId> {
        self.scenario
            .map
            .hexes
            .iter()
            .filter(|hex| self.hex_in_enemy_zoc(side_id, &hex.id))
            .map(|hex| hex.id.clone())
            .collect()
    }
}

/// Checks scenario turns, map references, ownership, and reinforcement strength-step setup.
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
