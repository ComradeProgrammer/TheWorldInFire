use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use ooaw_plugin_api::protocol::{
    PluginCall, ScenarioRequest, ScenarioSetup, DEBUG_COMMAND_PREFIX, END_PHASE_COMMAND,
};
use ooaw_plugin_api::serde_json::{self, Value};
use ooaw_plugin_api::{
    CityControlState, GameId, GameStatus, HexId, MapDefinition, PendingDecision, PhaseActor,
    PhaseDefinition, PhaseExecution, PhaseId, RuleError, ScenarioSummary, SideId, TurnState, Unit,
};
use serde::{Deserialize, Serialize};
use wasmtime::Store;

use crate::dice::Dice;
use crate::runtime::{dispatch, new_store, official_plugin, HostData, PluginModule, Sandbox, Slot};
use crate::state::{check_unit, KernelState};

/// Version of the serialized snapshot contract.
pub const PROTOCOL_VERSION: u16 = 17;

/// Complete client-facing representation of the authoritative game state.
///
/// Rules plugins add their own top-level entries (for example `battlePlan` or
/// `combat`), which serialize alongside the kernel's fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// All units in play.
    pub units: Vec<Unit>,
    /// Control of every city hex on the map.
    pub cities: Vec<CityControlState>,
    /// Player decision that must be resolved before play can continue.
    pub pending_decision: Option<PendingDecision>,
    /// Rules-owned state entries, flattened into the snapshot object.
    #[serde(flatten)]
    pub rules: BTreeMap<String, Value>,
}

/// The events and current snapshot produced by an accepted game command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandOutcome {
    /// The authoritative revision after the command has been applied.
    pub revision: u64,
    /// Ordered domain events, each a JSON object with a `type` tag.
    pub events: Vec<Value>,
    /// The complete authoritative state after command processing.
    pub snapshot: GameSnapshot,
}

/// Phase events emitted by the kernel's turn sequencer.
#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum KernelEvent<'a> {
    PhaseEnded {
        game_turn: u16,
        step: &'a PhaseDefinition,
    },
    PhaseStarted {
        game_turn: u16,
        step: &'a PhaseDefinition,
    },
    GameTurnStarted {
        game_turn: u16,
    },
    GameCompleted {
        game_turn: u16,
    },
}

impl KernelEvent<'_> {
    fn into_value(self) -> Value {
        serde_json::to_value(self).expect("kernel events serialize")
    }
}

/// Step at which a custom setup starts, from the setup document's `start` entry.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupStart {
    game_turn: u16,
    phase_id: PhaseId,
    #[serde(default)]
    side_id: Option<SideId>,
}

/// Which plugin handles each phase, command, and query.
#[derive(Default)]
struct Registry {
    phases: HashMap<String, usize>,
    commands: HashMap<String, usize>,
    queries: HashMap<String, usize>,
}

/// Authoritative game state, turn sequencer, and command router for one game.
///
/// The engine owns the state and the dice and advances the turn sequence.
/// What each phase and command does is decided by the loaded rules plugins,
/// which run as WebAssembly. A command either succeeds as a whole or leaves
/// the game exactly as it was.
pub struct GameEngine {
    /// Stable identifier for this game session and its initial dice seed.
    game_id: GameId,
    /// Scenario the game was created from.
    scenario_request: ScenarioRequest,
    /// Client-facing scenario metadata.
    scenario: ScenarioSummary,
    /// Authoritative map.
    map: MapDefinition,
    /// Ordered steps repeated every game turn.
    turn_sequence: Vec<PhaseDefinition>,
    /// Number of game turns after which the scenario ends.
    max_game_turns: u16,
    /// Original owner of every city hex.
    city_owners: BTreeMap<HexId, SideId>,
    /// Count of successfully executed commands.
    revision: u64,
    /// Whether `debug.*` commands are accepted.
    debug_commands: bool,
    /// Plugin that created the scenario.
    owner: usize,
    /// Handlers by phase, command, and query.
    registry: Registry,
    /// Plugin instances together with the authoritative state and dice.
    store: Store<HostData>,
}

impl GameEngine {
    /// Creates a game from a scenario of the bundled official rules plugin.
    ///
    /// The game opens on the first step that needs a player: the scenario's
    /// leading automatic phases are resolved and their events dropped.
    ///
    /// # Errors
    ///
    /// Returns `scenarioNotFound` for an unknown scenario, `invalidScenario` if
    /// its content is inconsistent, or a plugin error.
    pub fn new(game_id: GameId, scenario: impl Into<ScenarioRequest>) -> Result<Self, RuleError> {
        Self::with_plugins(game_id, vec![official_plugin()?], scenario.into(), None)
    }

    /// Creates a game from a bundled scenario and lays out a custom starting
    /// situation.
    ///
    /// The setup document is in the scenario plugin's own format. If it has a
    /// `start` entry (`gameTurn`, `phaseId`, and optional `sideId`), the engine
    /// first plays through the turn sequence to that step.
    ///
    /// # Errors
    ///
    /// Returns `invalidSetup` for a start step that is never reached or a
    /// setup the plugin rejects, in addition to the errors of [`Self::new`].
    pub fn with_setup(
        game_id: GameId,
        scenario: impl Into<ScenarioRequest>,
        setup: Value,
    ) -> Result<Self, RuleError> {
        Self::with_plugins(
            game_id,
            vec![official_plugin()?],
            scenario.into(),
            Some(setup),
        )
    }

    /// Creates a game with an explicit list of rules plugins, in load order.
    ///
    /// A later plugin overrides an earlier one's phases, commands, and queries,
    /// and every plugin listing a filter takes part in it. The scenario comes
    /// from the last plugin offering its ID.
    ///
    /// # Errors
    ///
    /// See [`Self::new`] and [`Self::with_setup`].
    pub fn with_plugins(
        game_id: GameId,
        plugins: Vec<Arc<PluginModule>>,
        scenario: ScenarioRequest,
        setup: Option<Value>,
    ) -> Result<Self, RuleError> {
        let owner = plugins
            .iter()
            .rposition(|plugin| {
                plugin
                    .manifest()
                    .scenarios
                    .iter()
                    .any(|summary| summary.id == scenario.id)
            })
            .ok_or_else(|| {
                RuleError::new(
                    "scenarioNotFound",
                    format!("Unknown scenario: {}", scenario.id),
                )
            })?;
        let mut registry = Registry::default();
        let mut filters: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, plugin) in plugins.iter().enumerate() {
            let manifest = plugin.manifest();
            for phase in &manifest.phases {
                registry.phases.insert(phase.clone(), index);
            }
            for command in &manifest.commands {
                registry.commands.insert(command.clone(), index);
            }
            for query in &manifest.queries {
                registry.queries.insert(query.clone(), index);
            }
            for filter in &manifest.filters {
                filters.entry(filter.clone()).or_default().push(index);
            }
        }

        let placeholder = KernelState::opening(Default::default());
        let sandbox = if plugins.iter().all(|plugin| plugin.is_trusted()) {
            Sandbox::Trusted
        } else {
            Sandbox::Guarded
        };
        let mut data = HostData::new(
            sandbox,
            placeholder,
            Dice::from_seed_text(&game_id.0),
            HashSet::new(),
        );
        data.slots = plugins.into_iter().map(Slot::new).collect();
        data.filters = filters;
        let mut store = new_store(data);

        let response = dispatch(
            &mut store,
            owner,
            PluginCall::CreateGame {
                game_id: game_id.clone(),
                scenario: scenario.clone(),
            },
        )?;
        let setup_value = response.result?;
        let content: ScenarioSetup = serde_json::from_value(setup_value).map_err(|error| {
            RuleError::protocol(format!("Malformed scenario from plugin: {error}"))
        })?;
        let ScenarioSetup {
            scenario: summary,
            map,
            turn_sequence,
            state,
        } = content;
        let hexes = validate_content(&summary, &map, &turn_sequence)?;
        for unit in &state.units {
            check_unit(unit, &hexes)
                .map_err(|error| RuleError::new("invalidScenario", error.message))?;
        }
        let city_owners = map
            .hexes
            .iter()
            .filter_map(|hex| {
                hex.city
                    .as_ref()
                    .map(|city| (hex.id.clone(), city.owner.clone()))
            })
            .collect();

        {
            let data = store.data_mut();
            data.state = KernelState::opening(state);
            data.hexes = hexes;
            data.invalidate();
            for (index, slot) in data.slots.iter_mut().enumerate() {
                slot.attach = Some(PluginCall::Attach {
                    game_id: game_id.clone(),
                    scenario: scenario.clone(),
                    summary: summary.clone(),
                    turn_sequence: turn_sequence.clone(),
                    map: (index != owner).then(|| Box::new(map.clone())),
                });
            }
        }
        for index in 0..store.data().slots.len() {
            if index != owner {
                let attach = store.data().slots[index].attach.clone().expect("set above");
                dispatch(&mut store, index, attach)?.result?;
            }
        }

        let mut game = Self {
            game_id,
            scenario_request: scenario,
            max_game_turns: summary.max_game_turns,
            scenario: summary,
            map,
            turn_sequence,
            city_owners,
            revision: 0,
            debug_commands: false,
            owner,
            registry,
            store,
        };
        // Resolve the scenario's leading automatic phases (such as Joint Status,
        // Joint Reinforcement, and Pre-Battle) so play opens on the first step
        // that needs a player. Their events precede any client and are dropped.
        let mut events = Vec::new();
        game.start_current_phase(&mut events)?;
        game.advance_automatic_steps(&mut events)?;
        if let Some(setup) = setup {
            game.apply_setup(setup)?;
        }
        Ok(game)
    }

    /// Plays to a setup's start step and passes the setup to the scenario plugin.
    fn apply_setup(&mut self, setup: Value) -> Result<(), RuleError> {
        let invalid = |message: String| RuleError::new("invalidSetup", message);
        if let Some(start) = setup.get("start").filter(|start| !start.is_null()) {
            let start: SetupStart = serde_json::from_value(start.clone())
                .map_err(|error| invalid(format!("Invalid start: {error}")))?;
            loop {
                let never = || {
                    invalid("The setup's start step is never reached in this scenario".to_owned())
                };
                let state = &self.store.data().state;
                if state.status == GameStatus::Completed || state.game_turn > start.game_turn {
                    return Err(never());
                }
                let step = self.current_step().expect("game in progress has a step");
                let side = match &step.actor {
                    PhaseActor::Side { side_id } => Some(side_id),
                    PhaseActor::All => None,
                };
                if state.game_turn == start.game_turn
                    && step.phase_id == start.phase_id
                    && side == start.side_id.as_ref()
                {
                    break;
                }
                self.end_phase()?;
            }
        }
        let response = dispatch(
            &mut self.store,
            self.owner,
            PluginCall::ApplySetup { setup },
        )?;
        response.result?;
        Ok(())
    }

    /// Returns an owned, serializable snapshot of the current game state.
    ///
    /// Static map data is represented by its scenario identifier; retrieve it
    /// with [`Self::map`].
    pub fn snapshot(&self) -> GameSnapshot {
        let state = &self.store.data().state;
        GameSnapshot {
            protocol_version: PROTOCOL_VERSION,
            game_id: self.game_id.clone(),
            revision: self.revision,
            scenario: self.scenario.clone(),
            status: state.status,
            turn: TurnState {
                game_turn: state.game_turn,
                step_index: state.step_index,
                current_step: self.current_step().cloned(),
            },
            units: state.units.values().cloned().collect(),
            cities: state
                .city_control
                .iter()
                .filter_map(|(hex_id, controller)| {
                    let owner = self.city_owners.get(hex_id)?.clone();
                    Some(CityControlState {
                        hex_id: hex_id.clone(),
                        free: owner == *controller,
                        owner,
                        controller: controller.clone(),
                    })
                })
                .collect(),
            pending_decision: state.pending_decision.clone(),
            rules: state.rules.clone(),
        }
    }

    /// Returns the number of commands executed so far.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns the authoritative map selected by this game's scenario.
    pub fn map(&self) -> &MapDefinition {
        &self.map
    }

    /// Returns the scenario's ordered phase steps repeated every game turn.
    pub fn turn_sequence(&self) -> &[PhaseDefinition] {
        &self.turn_sequence
    }

    /// Returns the scenario this game was created from.
    pub fn scenario_request(&self) -> &ScenarioRequest {
        &self.scenario_request
    }

    /// Returns the loaded plugins' manifests in load order.
    pub fn plugins(&self) -> Vec<&ooaw_plugin_api::protocol::PluginManifest> {
        self.store
            .data()
            .slots
            .iter()
            .map(|slot| slot.plugin.manifest())
            .collect()
    }

    /// Returns the active scenario step, or none once the game has completed.
    fn current_step(&self) -> Option<&PhaseDefinition> {
        let state = &self.store.data().state;
        if state.status == GameStatus::Completed {
            None
        } else {
            self.turn_sequence.get(state.step_index)
        }
    }

    /// Validates and executes a command against the authoritative game state.
    ///
    /// The command is a JSON object with a `type` tag. `endPhase` is handled by
    /// the kernel's turn sequencer; every other type goes to the plugin that
    /// declared it. A successful command increments the revision once and
    /// returns its ordered events and the resulting snapshot.
    ///
    /// # Errors
    ///
    /// Returns `gameComplete` after the scenario ends, `unknownCommand` for a
    /// type no plugin handles, or the handling plugin's rejection. A rejected
    /// command leaves state, dice, and revision unchanged.
    pub fn execute(&mut self, command: Value) -> Result<CommandOutcome, RuleError> {
        if self.store.data().state.status == GameStatus::Completed {
            return Err(RuleError::new(
                "gameComplete",
                "The game has already completed",
            ));
        }
        let kind = command
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| RuleError::new("invalidCommand", "A command needs a string type"))?
            .to_owned();
        if kind.starts_with(DEBUG_COMMAND_PREFIX) && !self.debug_commands {
            return Err(RuleError::new(
                "debugCommandsDisabled",
                "Debug commands are disabled in this game",
            ));
        }

        let backup = {
            let data = self.store.data();
            (data.state.clone(), data.dice.clone())
        };
        let result = if kind == END_PHASE_COMMAND {
            self.end_phase()
        } else {
            match self.registry.commands.get(&kind).copied() {
                Some(slot) => dispatch(&mut self.store, slot, PluginCall::Command { command })
                    .and_then(|response| response.result.map(|_| response.events)),
                None => Err(RuleError::new(
                    "unknownCommand",
                    format!("No rules plugin handles the command {kind}"),
                )),
            }
        };
        match result {
            Ok(events) => {
                self.revision += 1;
                Ok(CommandOutcome {
                    revision: self.revision,
                    events,
                    snapshot: self.snapshot(),
                })
            }
            Err(error) => {
                let data = self.store.data_mut();
                (data.state, data.dice) = backup;
                data.invalidate();
                Err(error)
            }
        }
    }

    /// Answers a read-only rules query, such as a movement or combat preview.
    ///
    /// Queries never change state or consume dice.
    ///
    /// # Errors
    ///
    /// Returns `unknownQuery` for a name no plugin answers, or the plugin's
    /// rejection.
    pub fn query(&mut self, name: &str, input: Value) -> Result<Value, RuleError> {
        let slot = self.registry.queries.get(name).copied().ok_or_else(|| {
            RuleError::new(
                "unknownQuery",
                format!("No rules plugin answers the query {name}"),
            )
        })?;
        self.store.data_mut().read_only = true;
        let response = dispatch(
            &mut self.store,
            slot,
            PluginCall::Query {
                name: name.to_owned(),
                input,
            },
        );
        self.store.data_mut().read_only = false;
        response?.result
    }

    /// Accepts `debug.*` commands, which rules plugins may provide for tests
    /// and development tools.
    pub fn enable_debug_commands(&mut self) {
        self.debug_commands = true;
    }

    /// Edits the authoritative state directly, bypassing every rule.
    ///
    /// Intended for tests and development tools. Every plugin's mirror is
    /// resynchronised before its next call.
    pub fn edit_state(&mut self, edit: impl FnOnce(&mut KernelState)) {
        let data = self.store.data_mut();
        edit(&mut data.state);
        data.invalidate();
    }

    /// Returns the authoritative state.
    pub fn state(&self) -> &KernelState {
        &self.store.data().state
    }

    /// Returns the game's dice.
    pub fn dice(&self) -> &Dice {
        &self.store.data().dice
    }

    /// Replaces the game's dice, for tests that need a specific roll.
    pub fn set_dice(&mut self, dice: Dice) {
        self.store.data_mut().dice = dice;
    }

    /// Ends the current phase and resolves automatic phases until input is
    /// needed or the game ends.
    fn end_phase(&mut self) -> Result<Vec<Value>, RuleError> {
        let mut events = Vec::new();
        if let Some(step) = self.current_step().cloned() {
            // The phase's plugin cleans up first, or rejects ending the phase.
            self.phase_hook(&step, false, &mut events)?;
            events.push(
                KernelEvent::PhaseEnded {
                    game_turn: self.store.data().state.game_turn,
                    step: &step,
                }
                .into_value(),
            );
        }
        self.move_to_next_step(&mut events)?;
        self.advance_automatic_steps(&mut events)?;
        Ok(events)
    }

    /// Automatic phases do their work when they start; clients never stop on
    /// them, so keep advancing until input is required again.
    fn advance_automatic_steps(&mut self, events: &mut Vec<Value>) -> Result<(), RuleError> {
        while let Some(step) = self
            .current_step()
            .filter(|step| matches!(step.execution, PhaseExecution::Automatic))
            .cloned()
        {
            self.phase_hook(&step, false, events)?;
            events.push(
                KernelEvent::PhaseEnded {
                    game_turn: self.store.data().state.game_turn,
                    step: &step,
                }
                .into_value(),
            );
            self.move_to_next_step(events)?;
        }
        Ok(())
    }

    /// Starts the next phase, rolls into a new turn, or completes the game
    /// after its final turn.
    fn move_to_next_step(&mut self, events: &mut Vec<Value>) -> Result<(), RuleError> {
        let sequence_len = self.turn_sequence.len();
        let max_game_turns = self.max_game_turns;
        let state = &mut self.store.data_mut().state;
        state.step_index += 1;
        if state.step_index >= sequence_len {
            if state.game_turn >= max_game_turns {
                state.status = GameStatus::Completed;
                state.step_index = sequence_len;
                events.push(
                    KernelEvent::GameCompleted {
                        game_turn: state.game_turn,
                    }
                    .into_value(),
                );
                return Ok(());
            }
            state.game_turn += 1;
            state.step_index = 0;
            events.push(
                KernelEvent::GameTurnStarted {
                    game_turn: state.game_turn,
                }
                .into_value(),
            );
        }
        if let Some(step) = self.current_step() {
            events.push(
                KernelEvent::PhaseStarted {
                    game_turn: self.store.data().state.game_turn,
                    step,
                }
                .into_value(),
            );
        }
        self.start_current_phase(events)
    }

    /// Runs the starting hook of the current phase.
    fn start_current_phase(&mut self, events: &mut Vec<Value>) -> Result<(), RuleError> {
        if let Some(step) = self.current_step().cloned() {
            self.phase_hook(&step, true, events)?;
        }
        Ok(())
    }

    /// Sends a phase's start or end to the plugin handling its phase type.
    fn phase_hook(
        &mut self,
        step: &PhaseDefinition,
        starting: bool,
        events: &mut Vec<Value>,
    ) -> Result<(), RuleError> {
        let Some(slot) = self.registry.phases.get(&step.phase_id.0).copied() else {
            return Ok(());
        };
        let phase = step.clone();
        let call = if starting {
            PluginCall::PhaseStarted { phase }
        } else {
            PluginCall::PhaseEnding { phase }
        };
        let response = dispatch(&mut self.store, slot, call)?;
        response.result?;
        events.extend(response.events);
        Ok(())
    }
}

/// Lists the scenarios of the bundled official rules plugin.
///
/// # Errors
///
/// Returns `pluginLoad` if the bundled plugin cannot be loaded.
pub fn list_scenarios() -> Result<Vec<ScenarioSummary>, RuleError> {
    Ok(official_plugin()?.manifest().scenarios.clone())
}

/// Checks the structural consistency of plugin-supplied scenario content and
/// returns the set of map hexes.
fn validate_content(
    summary: &ScenarioSummary,
    map: &MapDefinition,
    turn_sequence: &[PhaseDefinition],
) -> Result<HashSet<HexId>, RuleError> {
    let invalid = |message: String| RuleError::new("invalidScenario", message);
    if summary.max_game_turns == 0 {
        return Err(invalid(
            "A scenario must contain at least one game turn".to_owned(),
        ));
    }
    if turn_sequence.is_empty() {
        return Err(invalid(
            "A scenario must contain at least one turn step".to_owned(),
        ));
    }
    if map.hexes.is_empty() {
        return Err(invalid(
            "A scenario map must contain at least one hex".to_owned(),
        ));
    }
    let mut hexes = HashSet::new();
    for hex in &map.hexes {
        if !hexes.insert(hex.id.clone()) {
            return Err(invalid(format!("Duplicate map hex ID: {}", hex.id.0)));
        }
    }
    for step in turn_sequence {
        if let PhaseActor::Side { side_id } = &step.actor {
            if !summary.sides.iter().any(|side| side.id == *side_id) {
                return Err(invalid(format!(
                    "Turn step references unknown side: {}",
                    side_id.0
                )));
            }
        }
    }
    Ok(hexes)
}
