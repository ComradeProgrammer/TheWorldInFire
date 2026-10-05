//! The plugin entry point: manifest, scenarios, and call routing.

use ooaw_plugin_api::protocol::{
    Change, PluginManifest, ScenarioRequest, ScenarioSetup, ABI_VERSION,
};
use ooaw_plugin_api::serde_json::Value;
use ooaw_plugin_api::{GameId, PhaseDefinition, TurnPosition};
use ooaw_plugin_sdk::{Attachment, RulesPlugin};
use serde::Deserialize;

use crate::command::GameCommand;
use crate::error::RuleError;
use crate::event::GameEvent;
use crate::model::{
    find_scenario, list_scenarios, AirUnitId, HexId, MovementMode, ScenarioDefinition, SideId,
    UnitId,
};
use crate::rules::{decode, encode, Rules};
use crate::setup::GameSetup;

/// Identifier of the official NATO rules plugin.
pub const PLUGIN_ID: &str = "ooaw.nato";

/// Phase types whose start or end the NATO rules handle.
const PHASES: [&str; 9] = [
    "jointStatus",
    "jointReinforcement",
    "preBattle",
    "battlePlanning",
    "jointAirOperations",
    "offensiveStrike",
    "combat",
    "reserve",
    "postBattle",
];

/// Command `type` tags the NATO rules execute.
const COMMANDS: [&str; 13] = [
    "setResupplyTarget",
    "setAttackTarget",
    "moveUnit",
    "setReserve",
    "undoUnitMovement",
    "entrainUnit",
    "detrainUnit",
    "undoDetrainUnit",
    "planAirSortie",
    "cancelAirSortie",
    "resolveBattle",
    "advanceAfterCombat",
    "debug.checkSupply",
];

/// Read-only queries the NATO rules answer.
const QUERIES: [&str; 11] = [
    "movementModes",
    "movementOptions",
    "attackTargetOptions",
    "reserveOptions",
    "airStrikeOptions",
    "airPlanningOptions",
    "airMissionOptions",
    "combatOptions",
    "battlePreview",
    "enemyZoc",
    "airspace",
];

/// The official NATO rules as a [`RulesPlugin`].
#[derive(Default)]
pub struct NatoPlugin {
    /// Rules of the game this instance serves, once created or attached.
    rules: Option<Rules>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnitInput {
    unit_id: UnitId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AirUnitInput {
    air_unit_id: AirUnitId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MovementOptionsInput {
    unit_id: UnitId,
    mode: MovementMode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BattlePreviewInput {
    hex_id: HexId,
    unit_ids: Vec<UnitId>,
    #[serde(default)]
    supporting_hq_id: Option<UnitId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SideInput {
    side_id: SideId,
}

/// Looks up a registered scenario or decodes a supplied definition.
fn resolve_scenario(request: ScenarioRequest) -> Result<ScenarioDefinition, RuleError> {
    match request.definition {
        Some(definition) => serde_json::from_value(definition).map_err(|error| {
            RuleError::new(
                "invalidScenario",
                format!("Malformed scenario definition: {error}"),
            )
        }),
        None => find_scenario(&request.id).ok_or_else(|| {
            RuleError::new(
                "scenarioNotFound",
                format!("Unknown scenario: {}", request.id),
            )
        }),
    }
}

fn events(events: Vec<GameEvent>) -> Result<Vec<Value>, RuleError> {
    events.iter().map(encode).collect()
}

impl NatoPlugin {
    fn rules(&self) -> Result<&Rules, RuleError> {
        self.rules
            .as_ref()
            .ok_or_else(|| RuleError::protocol("The plugin is not attached to a game"))
    }

    fn rules_mut(&mut self) -> Result<&mut Rules, RuleError> {
        self.rules
            .as_mut()
            .ok_or_else(|| RuleError::protocol("The plugin is not attached to a game"))
    }
}

impl RulesPlugin for NatoPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: PLUGIN_ID.to_owned(),
            name: "NATO: The Cold War Goes Hot".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            abi_version: ABI_VERSION,
            stateless: false,
            phases: PHASES.iter().map(|name| (*name).to_owned()).collect(),
            commands: COMMANDS.iter().map(|name| (*name).to_owned()).collect(),
            queries: QUERIES.iter().map(|name| (*name).to_owned()).collect(),
            filters: Vec::new(),
            scenarios: list_scenarios(),
        }
    }

    fn create_game(
        &mut self,
        _game_id: GameId,
        scenario: ScenarioRequest,
    ) -> Result<ScenarioSetup, RuleError> {
        let rules = Rules::new(resolve_scenario(scenario)?)?;
        let setup = rules.scenario_setup()?;
        self.rules = Some(rules);
        Ok(setup)
    }

    fn attach(&mut self, attachment: Attachment) -> Result<(), RuleError> {
        self.rules = Some(Rules::new(resolve_scenario(attachment.scenario)?)?);
        Ok(())
    }

    fn set_turn(&mut self, turn: TurnPosition) {
        if let Some(rules) = &mut self.rules {
            rules.set_turn(turn);
        }
    }

    fn sync(&mut self, state: Value) -> Result<(), RuleError> {
        self.rules_mut()?.sync(state)
    }

    fn take_changes(&self) -> Result<Vec<Change>, RuleError> {
        match &self.rules {
            Some(rules) => rules.take_changes(),
            None => Ok(Vec::new()),
        }
    }

    fn apply_setup(&mut self, setup: Value) -> Result<Vec<Value>, RuleError> {
        let setup: GameSetup = serde_json::from_value(setup)
            .map_err(|error| RuleError::new("invalidSetup", error.to_string()))?;
        self.rules_mut()?.apply_setup(&setup)?;
        Ok(Vec::new())
    }

    fn phase_started(&mut self, _phase: &PhaseDefinition) -> Result<Vec<Value>, RuleError> {
        let mut started = Vec::new();
        self.rules_mut()?.on_phase_started(&mut started);
        events(started)
    }

    fn phase_ending(&mut self, phase: &PhaseDefinition) -> Result<Vec<Value>, RuleError> {
        events(self.rules_mut()?.phase_ending(&phase.phase_id.0)?)
    }

    fn command(&mut self, command: Value) -> Result<Vec<Value>, RuleError> {
        let command: GameCommand = serde_json::from_value(command)
            .map_err(|error| RuleError::new("invalidCommand", error.to_string()))?;
        events(self.rules_mut()?.execute(command)?)
    }

    fn query(&self, name: &str, input: Value) -> Result<Value, RuleError> {
        let rules = self.rules()?;
        match name {
            "movementModes" => {
                let input: UnitInput = decode(input)?;
                encode(&rules.movement_modes(&input.unit_id)?)
            }
            "movementOptions" => {
                let input: MovementOptionsInput = decode(input)?;
                encode(&rules.movement_options(&input.unit_id, input.mode)?)
            }
            "attackTargetOptions" => encode(&rules.attack_target_options()?),
            "reserveOptions" => encode(&rules.reserve_options()?),
            "airStrikeOptions" => encode(&rules.air_strike_options()?),
            "airPlanningOptions" => encode(&rules.air_planning_options()?),
            "airMissionOptions" => {
                let input: AirUnitInput = decode(input)?;
                encode(&rules.air_mission_options(&input.air_unit_id)?)
            }
            "combatOptions" => encode(&rules.combat_options()?),
            "battlePreview" => {
                let input: BattlePreviewInput = decode(input)?;
                encode(&rules.battle_preview(
                    &input.hex_id,
                    &input.unit_ids,
                    input.supporting_hq_id.as_ref(),
                )?)
            }
            "enemyZoc" => {
                let input: SideInput = decode(input)?;
                encode(&rules.enemy_zoc_hexes(&input.side_id))
            }
            "airspace" => {
                let input: SideInput = decode(input)?;
                encode(&rules.airspace_by_hex(&input.side_id))
            }
            _ => Err(RuleError::new(
                "unknownQuery",
                format!("The NATO rules answer no query {name}"),
            )),
        }
    }
}
