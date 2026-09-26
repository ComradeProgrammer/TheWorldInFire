use std::sync::Mutex;

use ooaw_core::{
    find_scenario, GameCommand, GameId, GameSnapshot, GameState, RuleError, ScenarioSummary,
};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Default)]
struct AppState {
    game: Mutex<Option<GameState>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiError {
    code: String,
    message: String,
}

impl ApiError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

impl From<RuleError> for ApiError {
    fn from(value: RuleError) -> Self {
        Self {
            code: value.code,
            message: value.message,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandRequest {
    expected_revision: u64,
    command: GameCommand,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandResponse {
    revision: u64,
    events: Vec<ooaw_core::GameEvent>,
    snapshot: GameSnapshot,
}

#[tauri::command]
fn list_scenarios() -> Vec<ScenarioSummary> {
    ooaw_core::list_scenarios()
}

#[tauri::command]
fn new_game(scenario_id: String, state: State<'_, AppState>) -> Result<GameSnapshot, ApiError> {
    let scenario = find_scenario(&scenario_id).ok_or_else(|| {
        ApiError::new(
            "scenarioNotFound",
            format!("Unknown scenario: {scenario_id}"),
        )
    })?;
    let game = GameState::new(GameId(uuid::Uuid::new_v4().to_string()), scenario)?;
    let snapshot = game.snapshot();
    let mut session = state
        .game
        .lock()
        .map_err(|_| ApiError::new("sessionUnavailable", "Game session lock is poisoned"))?;
    *session = Some(game);
    Ok(snapshot)
}

#[tauri::command]
fn get_game_snapshot(state: State<'_, AppState>) -> Result<GameSnapshot, ApiError> {
    let session = state
        .game
        .lock()
        .map_err(|_| ApiError::new("sessionUnavailable", "Game session lock is poisoned"))?;
    session
        .as_ref()
        .map(GameState::snapshot)
        .ok_or_else(|| ApiError::new("gameNotStarted", "No game has been started"))
}

#[tauri::command]
fn submit_game_command(
    request: CommandRequest,
    state: State<'_, AppState>,
) -> Result<CommandResponse, ApiError> {
    let mut session = state
        .game
        .lock()
        .map_err(|_| ApiError::new("sessionUnavailable", "Game session lock is poisoned"))?;
    let game = session
        .as_mut()
        .ok_or_else(|| ApiError::new("gameNotStarted", "No game has been started"))?;
    let actual_revision = game.snapshot().revision;
    if request.expected_revision != actual_revision {
        return Err(ApiError::new(
            "revisionMismatch",
            format!(
                "Expected revision {}, but the current revision is {actual_revision}",
                request.expected_revision
            ),
        ));
    }
    let outcome = game.execute(request.command)?;
    Ok(CommandResponse {
        revision: outcome.revision,
        events: outcome.events,
        snapshot: outcome.snapshot,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Builds and starts the Tauri desktop application.
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_scenarios,
            new_game,
            get_game_snapshot,
            submit_game_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
