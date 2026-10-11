//! Types in the signatures of [`RulesPlugin`](crate::sdk::RulesPlugin) that the
//! kernel also uses: what a plugin provides, the scenarios it builds, and the
//! changes it makes to the game state.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    GameId, GameState, HexId, MapDefinition, PendingDecision, PhaseDefinition, ScenarioSummary,
    SideId, Unit, UnitId,
};

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

/// What a plugin provides, returned by [`RulesPlugin::manifest`](crate::sdk::RulesPlugin::manifest).
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
    /// [`ABI_VERSION`](crate::ABI_VERSION) the plugin was built against.
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
    /// Filters the plugin takes part in (see [`RulesPlugin::filter`](crate::sdk::RulesPlugin::filter)).
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

/// The game a plugin is attached to by [`RulesPlugin::attach`](crate::sdk::RulesPlugin::attach).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// Game identifier.
    pub game_id: GameId,
    /// Scenario the game was created from.
    pub scenario: ScenarioRequest,
    /// Client-facing scenario metadata.
    pub summary: ScenarioSummary,
    /// Turn sequence of the game.
    pub turn_sequence: Vec<PhaseDefinition>,
    /// The map, present when the plugin does not own the scenario.
    pub map: Option<Box<MapDefinition>>,
}

/// Static content and opening state returned by [`RulesPlugin::create_game`](crate::sdk::RulesPlugin::create_game).
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
