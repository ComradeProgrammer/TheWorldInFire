//! Messages exchanged between the kernel and rules plugins.
//!
//! # Binary interface (ABI version 1)
//!
//! A rules plugin is a core WebAssembly module with no WASI imports. Every
//! message is UTF-8 JSON. The module exports:
//!
//! - `memory`: its linear memory.
//! - `ooaw_abi_version() -> i32`: returns [`ABI_VERSION`].
//! - `ooaw_alloc(len: i32) -> i32`: allocates `len` bytes for the kernel to write a request into.
//! - `ooaw_free(ptr: i32, len: i32)`: frees a buffer from `ooaw_alloc` or a response.
//! - `ooaw_dispatch(ptr: i32, len: i32) -> i64`: handles the [`DispatchRequest`]
//!   at `ptr` and frees it, then returns `(response_ptr << 32) | response_len`
//!   for a [`DispatchResponse`] the kernel frees with `ooaw_free`.
//!
//! It may import from module `ooaw`:
//!
//! - `roll(sides: i32) -> i32`: the next value, `1..=sides`, of the game's
//!   seeded dice. Traps during queries and filters, which must not consume dice.
//! - `host_call(ptr: i32, len: i32) -> i32`: sends a [`HostRequest`] and returns
//!   the byte length of its JSON [`HostResponse`].
//! - `host_read(ptr: i32)`: copies that pending response into guest memory.
//!
//! The kernel never re-enters a plugin that is already handling a call.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    GameId, GameState, HexId, MapDefinition, PendingDecision, PhaseDefinition, RuleError,
    ScenarioSummary, SideId, TurnPosition, Unit, UnitId,
};

/// Version of the binary interface described in this module.
pub const ABI_VERSION: u32 = 1;

/// Command type handled by the kernel itself rather than by a plugin.
pub const END_PHASE_COMMAND: &str = "endPhase";

/// Prefix of commands the kernel accepts only when debug commands are enabled.
pub const DEBUG_COMMAND_PREFIX: &str = "debug.";

/// Top-level snapshot fields owned by the kernel; plugins may not use them as
/// rules-state keys.
pub const RESERVED_STATE_KEYS: [&str; 9] = [
    "protocolVersion",
    "gameId",
    "revision",
    "scenario",
    "status",
    "turn",
    "units",
    "cities",
    "pendingDecision",
];

/// What a plugin provides, returned by [`PluginCall::Manifest`].
///
/// When several loaded plugins claim the same phase, command, or query, the one
/// loaded last handles it. Every plugin that lists a filter takes part in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// Stable plugin identifier, such as `ooaw.nato`.
    pub id: String,
    /// Human-readable plugin name.
    pub name: String,
    /// Plugin release version.
    pub version: String,
    /// [`ABI_VERSION`] the plugin was built against.
    pub abi_version: u32,
    /// Whether the plugin keeps no mirror of the game state. The kernel then
    /// never sends it the state, and it may report no changes.
    #[serde(default)]
    pub stateless: bool,
    /// Phase types (`PhaseDefinition::phase_id`) whose start and end the plugin handles.
    #[serde(default)]
    pub phases: Vec<String>,
    /// Command `type` tags the plugin executes.
    #[serde(default)]
    pub commands: Vec<String>,
    /// Read-only queries the plugin answers.
    #[serde(default)]
    pub queries: Vec<String>,
    /// Filters the plugin takes part in (see [`PluginCall::Filter`]).
    #[serde(default)]
    pub filters: Vec<String>,
    /// Scenarios the plugin can create.
    #[serde(default)]
    pub scenarios: Vec<ScenarioSummary>,
}

/// Scenario to create or attach to: a registered ID, or a complete definition
/// in the owning plugin's own format (used by tests and editors).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioRequest {
    /// Scenario identifier.
    pub id: String,
    /// Full scenario definition replacing the registered one, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<Value>,
}

impl From<&str> for ScenarioRequest {
    fn from(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            definition: None,
        }
    }
}

/// Static content and opening state returned by [`PluginCall::CreateGame`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioSetup {
    /// Client-facing scenario metadata.
    pub scenario: ScenarioSummary,
    /// Authoritative map.
    pub map: MapDefinition,
    /// Ordered steps repeated every game turn.
    pub turn_sequence: Vec<PhaseDefinition>,
    /// State before the first phase starts.
    pub state: GameState,
}

/// One kernel-to-plugin message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchRequest<S = GameState> {
    /// Current turn position; absent outside a game.
    #[serde(default)]
    pub turn: Option<TurnPosition>,
    /// Complete mutable state, present when the plugin's cached copy is stale.
    #[serde(default)]
    pub sync: Option<S>,
    /// Filters that another plugin takes part in. A plugin may skip
    /// [`HostRequest::Filter`] for any filter not listed.
    #[serde(default)]
    pub active_filters: Vec<String>,
    /// Operation to perform.
    pub call: PluginCall,
}

/// Operation requested from a plugin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "call",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PluginCall {
    /// Returns the [`PluginManifest`]. Called once, outside any game.
    Manifest,
    /// Builds a scenario the plugin owns and returns its [`ScenarioSetup`].
    /// The plugin is attached to the new game afterwards.
    CreateGame {
        /// New game's identifier.
        game_id: GameId,
        /// Scenario to build.
        scenario: ScenarioRequest,
    },
    /// Attaches a fresh plugin instance to an existing game, for example after
    /// loading a save or recovering from a fault. Returns nothing.
    Attach {
        /// Game identifier.
        game_id: GameId,
        /// Scenario the game was created from.
        scenario: ScenarioRequest,
        /// Client-facing scenario metadata.
        summary: ScenarioSummary,
        /// Turn sequence of the game.
        turn_sequence: Vec<PhaseDefinition>,
        /// The game's map, sent to plugins that do not own the scenario.
        #[serde(default)]
        map: Option<Box<MapDefinition>>,
    },
    /// Lays out a custom starting situation in the owning plugin's format.
    /// Sent to the scenario owner after the kernel reaches the setup's start step.
    ApplySetup {
        /// Setup document.
        setup: Value,
    },
    /// A phase the plugin handles has become current.
    PhaseStarted {
        /// The phase.
        phase: PhaseDefinition,
    },
    /// A phase the plugin handles is about to end. Rejecting keeps the phase current.
    PhaseEnding {
        /// The phase.
        phase: PhaseDefinition,
    },
    /// Executes a player command whose `type` the plugin declared.
    Command {
        /// Command object, including its `type` tag.
        command: Value,
    },
    /// Answers a read-only query. Changes and dice rolls are rejected.
    Query {
        /// Query name.
        name: String,
        /// Query input.
        input: Value,
    },
    /// Takes part in a filter: returns `value`, possibly adjusted. Changes and
    /// dice rolls are rejected.
    ///
    /// A filter is a named extension point. The plugin that needs the value
    /// computes a base value itself, then the kernel passes it through every
    /// other plugin listing the filter, in load order.
    Filter {
        /// Filter name.
        name: String,
        /// Context the caller supplied.
        input: Value,
        /// Value produced so far.
        value: Value,
    },
}

/// A plugin's reply to one [`DispatchRequest`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchResponse {
    /// Call result: `{"Ok": value}` or `{"Err": {"code", "message"}}`.
    pub result: Result<Value, RuleError>,
    /// State changes made by the call, which the kernel applies when the call succeeds.
    #[serde(default)]
    pub changes: Vec<Change>,
    /// Domain events, each a JSON object with a `type` tag, in order.
    #[serde(default)]
    pub events: Vec<Value>,
}

/// One change to the kernel's mutable state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Change {
    /// Adds a unit, or replaces one with the same identifier.
    PutUnit {
        /// Complete unit.
        unit: Unit,
    },
    /// Replaces top-level fields of an existing unit. Fields not listed keep
    /// their values, including markers other plugins own.
    UpdateUnit {
        /// Unit.
        unit_id: UnitId,
        /// Field values to replace.
        fields: Map<String, Value>,
    },
    /// Removes a unit from play.
    RemoveUnit {
        /// Unit.
        unit_id: UnitId,
    },
    /// Changes the controller of a city hex.
    SetCityControl {
        /// City hex.
        hex_id: HexId,
        /// New controller.
        side_id: SideId,
    },
    /// Replaces one rules-state entry; `null` keeps the key with a null value.
    SetRules {
        /// Entry name, which may not be one of [`RESERVED_STATE_KEYS`].
        key: String,
        /// New value.
        value: Value,
    },
    /// Sets or clears the decision players must make next.
    SetPendingDecision {
        /// New decision.
        decision: Option<PendingDecision>,
    },
}

/// One plugin-to-kernel request made during a call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "request",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum HostRequest {
    /// Runs a filter through the other plugins taking part in it.
    ///
    /// `changes` are the caller's changes so far in this call. The kernel
    /// applies them first so that other plugins see the current state.
    Filter {
        /// Filter name.
        name: String,
        /// Context for the other plugins.
        input: Value,
        /// Value the caller computed.
        value: Value,
        /// Caller's changes not yet sent.
        #[serde(default)]
        changes: Vec<Change>,
    },
    /// Writes a diagnostic message to the kernel's log.
    Log {
        /// Message text.
        message: String,
    },
}

/// The kernel's reply to a [`HostRequest`].
pub type HostResponse = Result<Value, RuleError>;
