use std::sync::Mutex;

use ooaw_core::api::serde_json::Value;
use ooaw_core::{
    GameEngine, GameId, GameSnapshot, MapDefinition, PhaseDefinition, RuleError, ScenarioSummary,
};
use serde::{Deserialize, Serialize};
use tauri::State;
#[derive(Default)]
struct AppState {
    game: Mutex<Option<GameEngine>>,
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
    /// Command object with a `type` tag, executed by the kernel or a rules plugin.
    command: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandResponse {
    revision: u64,
    events: Vec<Value>,
    snapshot: GameSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NewGameResponse {
    snapshot: GameSnapshot,
    map: MapDefinition,
    turn_sequence: Vec<PhaseDefinition>,
}

/// Closes the application from the title screen's Quit button.
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn list_scenarios() -> Result<Vec<ScenarioSummary>, ApiError> {
    Ok(ooaw_core::list_scenarios()?)
}

#[tauri::command]
fn new_game(scenario_id: String, state: State<'_, AppState>) -> Result<NewGameResponse, ApiError> {
    let game = GameEngine::new(
        GameId(uuid::Uuid::new_v4().to_string()),
        scenario_id.as_str(),
    )?;
    let snapshot = game.snapshot();
    let map = game.map().clone();
    let turn_sequence = game.turn_sequence().to_vec();
    let mut session = state
        .game
        .lock()
        .map_err(|_| ApiError::new("sessionUnavailable", "Game session lock is poisoned"))?;
    *session = Some(game);
    Ok(NewGameResponse {
        snapshot,
        map,
        turn_sequence,
    })
}

#[tauri::command]
fn get_game_snapshot(state: State<'_, AppState>) -> Result<GameSnapshot, ApiError> {
    let session = state
        .game
        .lock()
        .map_err(|_| ApiError::new("sessionUnavailable", "Game session lock is poisoned"))?;
    session
        .as_ref()
        .map(GameEngine::snapshot)
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
    let actual_revision = game.revision();
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RulesQueryRequest {
    /// Query name declared by a rules plugin, such as `battlePreview`.
    name: String,
    /// Query input; `null` when the query takes none.
    #[serde(default)]
    input: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RulesQueryResponse {
    revision: u64,
    result: Value,
}

/// Read-only rules preview, such as movement options or battle odds, answered
/// by the rules plugin that declared the query.
#[tauri::command]
fn rules_query(
    request: RulesQueryRequest,
    state: State<'_, AppState>,
) -> Result<RulesQueryResponse, ApiError> {
    let mut session = state
        .game
        .lock()
        .map_err(|_| ApiError::new("sessionUnavailable", "Game session lock is poisoned"))?;
    let game = session
        .as_mut()
        .ok_or_else(|| ApiError::new("gameNotStarted", "No game has been started"))?;
    let result = game.query(&request.name, request.input)?;
    Ok(RulesQueryResponse {
        revision: game.revision(),
        result,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Builds and starts the Tauri desktop application.
pub fn run() {
    // Compile the bundled rules plugin while the title screen loads.
    std::thread::spawn(|| {
        if let Err(error) = ooaw_core::official_plugin() {
            eprintln!("cannot load the official rules plugin: {error}");
        }
    });
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            quit_app,
            list_scenarios,
            new_game,
            get_game_snapshot,
            submit_game_command,
            rules_query
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
