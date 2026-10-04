use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{HexId, PhaseDefinition, SideDefinition, SideId, Unit};

/// High-level lifecycle state of a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameStatus {
    /// Commands may still advance the game.
    InProgress,
    /// The scenario has ended and no further game commands are accepted.
    Completed,
}

/// Position in the turn sequence, sent with every plugin call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPosition {
    /// One-based game-turn number.
    pub game_turn: u16,
    /// Zero-based index into the scenario's turn sequence.
    pub step_index: usize,
    /// Whether the game is still in progress.
    pub status: GameStatus,
}

/// Current position within a scenario's turn sequence, as shown to clients.
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

/// Description of an input currently required from a player.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDecision {
    /// Stable identifier for the kind of decision the client must present.
    pub kind: String,
}

/// Client-facing scenario metadata that omits internal setup details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioSummary {
    /// Stable identifier accepted when creating a game.
    pub id: String,
    /// Human-readable scenario title.
    pub name: String,
    /// Number of game turns in the scenario.
    pub max_game_turns: u16,
    /// Stable identifier of the authoritative map selected by the scenario.
    pub map_id: String,
    /// Sides available in the scenario.
    pub sides: Vec<SideDefinition>,
}

/// Control of one city hex.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CityControlState {
    /// City hex.
    pub hex_id: HexId,
    /// Alliance that controlled the city at the start of play.
    pub owner: SideId,
    /// Alliance that controls it now.
    pub controller: SideId,
    /// Whether it is a Free City (still held by its original alliance).
    pub free: bool,
}

/// The mutable game state a rules plugin mirrors.
///
/// The turn position travels separately with every call; this is the part a
/// plugin caches between calls and that the kernel resends only when the
/// plugin's copy is stale.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    /// Units in play.
    pub units: Vec<Unit>,
    /// Current controller of every city hex.
    pub city_control: BTreeMap<HexId, SideId>,
    /// Player decision that must be made before play continues.
    pub pending_decision: Option<PendingDecision>,
    /// Rules-owned top-level state keyed by name, such as `battlePlan`.
    pub rules: BTreeMap<String, Value>,
}
