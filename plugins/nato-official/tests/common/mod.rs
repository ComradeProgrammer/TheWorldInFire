//! Test harness: a NATO game run by the kernel with the bundled WebAssembly
//! plugin, seen through the rules' typed model.
//!
//! Every command and query goes through the real kernel and plugin. The
//! `edit_*` helpers change the kernel's state directly, bypassing the rules,
//! to lay out positions.

#![allow(dead_code)]

use std::collections::BTreeMap;

use ooaw_core::api::protocol::ScenarioRequest;
use ooaw_core::api::serde_json::{self, Value};
use ooaw_core::api::{
    CityControlState, GameStatus, PendingDecision, ScenarioSummary, TurnState, Unit,
};
use ooaw_core::{Dice, GameEngine, GameId, MapDefinition};
use ooaw_nato::{
    find_scenario, AirInterdictionZone, AirPoints, AirStrikeOptions, Airspace, BattleOdds,
    BattlePlan, BreakthroughMarker, CombatOptions, CombatState, GameCommand, GameEvent, GameSetup,
    HexId, MovementMode, MovementModeOptions, MovementOption, PhaseActor, ReserveOption,
    ReserveState, RuleError, RulesState, ScenarioDefinition, SideId, StrikePlan, UnitId, UnitState,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// The kernel snapshot of a NATO game, typed.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSnapshot {
    pub protocol_version: u16,
    pub game_id: GameId,
    pub revision: u64,
    pub scenario: ScenarioSummary,
    pub status: GameStatus,
    pub turn: TurnState,
    pub units: Vec<UnitState>,
    pub cities: Vec<CityControlState>,
    pub pending_decision: Option<PendingDecision>,
    pub battle_plans: Vec<BattlePlan>,
    /// The acting side's plan for the current turn, derived from `battle_plans`.
    #[serde(skip)]
    pub battle_plan: Option<BattlePlan>,
    pub air_points: Vec<AirPoints>,
    pub strike_plan: Option<StrikePlan>,
    pub air_interdiction_zones: Vec<AirInterdictionZone>,
    pub breakthrough_markers: Vec<BreakthroughMarker>,
    pub eliminated_unit_ids: Vec<UnitId>,
    pub combat: Option<CombatState>,
    pub reserve: Option<ReserveState>,
}

/// An accepted command's typed events and resulting snapshot.
#[derive(Clone, Debug)]
pub struct CommandOutcome {
    pub revision: u64,
    pub events: Vec<GameEvent>,
    pub snapshot: GameSnapshot,
}

/// Decodes a kernel snapshot and derives the acting side's battle plan.
fn decode_snapshot(value: Value) -> GameSnapshot {
    let mut snapshot: GameSnapshot = decode(value);
    let acting = snapshot
        .turn
        .current_step
        .as_ref()
        .and_then(|step| match &step.actor {
            PhaseActor::Side { side_id } => Some(side_id.clone()),
            PhaseActor::All => None,
        });
    snapshot.battle_plan = snapshot
        .battle_plans
        .iter()
        .find(|plan| {
            Some(&plan.side_id) == acting.as_ref() && plan.game_turn == snapshot.turn.game_turn
        })
        .cloned();
    snapshot
}

fn decode<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("kernel data matches the NATO model")
}

fn encode<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("NATO data serializes")
}

/// Requests a registered scenario by ID when it is unchanged, which avoids
/// sending the whole definition to the plugin.
fn scenario_request(scenario: &ScenarioDefinition) -> ScenarioRequest {
    if find_scenario(&scenario.id).as_ref() == Some(scenario) {
        ScenarioRequest::from(scenario.id.as_str())
    } else {
        ScenarioRequest {
            id: scenario.id.clone(),
            definition: Some(encode(scenario)),
        }
    }
}

/// A NATO game driven through the kernel.
pub struct TestGame {
    engine: GameEngine,
}

impl TestGame {
    /// Creates a game; debug commands are enabled.
    pub fn new(game_id: GameId, scenario: ScenarioDefinition) -> Result<Self, RuleError> {
        let mut engine = GameEngine::new(game_id, scenario_request(&scenario))?;
        engine.enable_debug_commands();
        Ok(Self { engine })
    }

    /// Creates a game from a custom setup; debug commands are enabled.
    pub fn with_setup(
        game_id: GameId,
        scenario: ScenarioDefinition,
        setup: &GameSetup,
    ) -> Result<Self, RuleError> {
        let mut engine =
            GameEngine::with_setup(game_id, scenario_request(&scenario), encode(setup))?;
        engine.enable_debug_commands();
        Ok(Self { engine })
    }

    pub fn execute(&mut self, command: GameCommand) -> Result<CommandOutcome, RuleError> {
        let outcome = self.engine.execute(encode(&command))?;
        Ok(CommandOutcome {
            revision: outcome.revision,
            events: outcome.events.into_iter().map(decode).collect(),
            snapshot: decode_snapshot(encode(&outcome.snapshot)),
        })
    }

    /// The kernel snapshot as clients receive it.
    pub fn snapshot_json(&self) -> Value {
        encode(&self.engine.snapshot())
    }

    pub fn snapshot(&self) -> GameSnapshot {
        decode_snapshot(encode(&self.engine.snapshot()))
    }

    pub fn map(&self) -> &MapDefinition {
        self.engine.map()
    }

    fn query<T: DeserializeOwned>(&mut self, name: &str, input: Value) -> Result<T, RuleError> {
        self.engine.query(name, input).map(decode)
    }

    pub fn movement_modes(
        &mut self,
        unit_id: &UnitId,
    ) -> Result<Vec<MovementModeOptions>, RuleError> {
        self.query("movementModes", json!({ "unitId": unit_id }))
    }

    pub fn movement_options(
        &mut self,
        unit_id: &UnitId,
        mode: MovementMode,
    ) -> Result<Vec<MovementOption>, RuleError> {
        self.query(
            "movementOptions",
            json!({ "unitId": unit_id, "mode": mode }),
        )
    }

    pub fn attack_target_options(&mut self) -> Result<Vec<HexId>, RuleError> {
        self.query("attackTargetOptions", Value::Null)
    }

    pub fn reserve_options(&mut self) -> Result<Vec<ReserveOption>, RuleError> {
        self.query("reserveOptions", Value::Null)
    }

    pub fn air_strike_options(&mut self) -> Result<AirStrikeOptions, RuleError> {
        self.query("airStrikeOptions", Value::Null)
    }

    pub fn combat_options(&mut self) -> Result<CombatOptions, RuleError> {
        self.query("combatOptions", Value::Null)
    }

    pub fn battle_preview(
        &mut self,
        hex_id: &HexId,
        unit_ids: &[UnitId],
        supporting_hq_id: Option<&UnitId>,
    ) -> Result<BattleOdds, RuleError> {
        self.query(
            "battlePreview",
            json!({ "hexId": hex_id, "unitIds": unit_ids, "supportingHqId": supporting_hq_id }),
        )
    }

    /// Whether `hex_id` lies in a zone of control of `side_id`'s enemies.
    pub fn hex_in_enemy_zoc(&mut self, side_id: &SideId, hex_id: &HexId) -> bool {
        let hexes: Vec<HexId> = self
            .query("enemyZoc", json!({ "sideId": side_id }))
            .unwrap();
        hexes.contains(hex_id)
    }

    /// Airspace of every hex from `side_id`'s point of view.
    pub fn airspace(&mut self, side_id: &SideId) -> BTreeMap<HexId, Airspace> {
        self.query("airspace", json!({ "sideId": side_id }))
            .unwrap()
    }

    /// Runs one side's supply check now, as the Pre-Battle Phase does.
    pub fn check_supply(&mut self, side_id: &str) -> Vec<GameEvent> {
        self.execute(GameCommand::DebugCheckSupply {
            side_id: SideId(side_id.to_owned()),
        })
        .unwrap()
        .events
    }

    /// Units in play, keyed by identifier.
    pub fn units(&self) -> BTreeMap<UnitId, UnitState> {
        self.engine
            .state()
            .units
            .iter()
            .map(|(id, unit)| (id.clone(), decode(encode(unit))))
            .collect()
    }

    /// One unit in play.
    pub fn unit(&self, unit_id: &UnitId) -> UnitState {
        decode(encode(&self.engine.state().units[unit_id]))
    }

    /// Edits the units in play directly.
    pub fn edit_units(&mut self, edit: impl FnOnce(&mut BTreeMap<UnitId, UnitState>)) {
        let mut units = self.units();
        edit(&mut units);
        let units: BTreeMap<UnitId, Unit> = units
            .into_iter()
            .map(|(id, unit)| (id, decode(encode(&unit))))
            .collect();
        self.engine.edit_state(|state| state.units = units);
    }

    /// Edits one unit in play directly.
    pub fn edit_unit(&mut self, unit_id: &UnitId, edit: impl FnOnce(&mut UnitState)) {
        self.edit_units(|units| edit(units.get_mut(unit_id).expect("unit in play")));
    }

    /// Removes a unit from play directly.
    pub fn remove_unit(&mut self, unit_id: &UnitId) {
        self.edit_units(|units| {
            units.remove(unit_id);
        });
    }

    /// Current controller of every city hex.
    pub fn city_control(&self) -> BTreeMap<HexId, SideId> {
        self.engine.state().city_control.clone()
    }

    /// Edits city control directly.
    pub fn edit_city_control(&mut self, edit: impl FnOnce(&mut BTreeMap<HexId, SideId>)) {
        self.engine
            .edit_state(|state| edit(&mut state.city_control));
    }

    /// Edits the NATO rules' own state directly.
    pub fn edit_rules(&mut self, edit: impl FnOnce(&mut RulesState)) {
        let mut rules: RulesState = decode(encode(&self.engine.state().rules));
        edit(&mut rules);
        let Value::Object(entries) = encode(&rules) else {
            unreachable!("rules state is an object");
        };
        self.engine.edit_state(|state| state.rules.extend(entries));
    }

    pub fn dice(&self) -> Dice {
        self.engine.dice().clone()
    }

    pub fn set_dice(&mut self, dice: Dice) {
        self.engine.set_dice(dice);
    }
}
