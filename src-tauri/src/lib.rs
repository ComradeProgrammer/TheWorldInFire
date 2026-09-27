use std::sync::Mutex;

use ooaw_core::{
    find_scenario, AirStrikeOptions, BattleOdds, CombatOptions, GameCommand, GameId, GameSnapshot, GameState, HexId, MapDefinition,
    MovementModeOptions, RuleError, ScenarioSummary, UnitId,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NewGameResponse {
    snapshot: GameSnapshot,
    map: MapDefinition,
}

#[tauri::command]
fn list_scenarios() -> Vec<ScenarioSummary> {
    ooaw_core::list_scenarios()
}

#[tauri::command]
fn new_game(scenario_id: String, state: State<'_, AppState>) -> Result<NewGameResponse, ApiError> {
    let scenario = find_scenario(&scenario_id).ok_or_else(|| {
        ApiError::new(
            "scenarioNotFound",
            format!("Unknown scenario: {scenario_id}"),
        )
    })?;
    let game = GameState::new(GameId(uuid::Uuid::new_v4().to_string()), scenario)?;
    let snapshot = game.snapshot();
    let map = game.map().clone();
    let mut session = state
        .game
        .lock()
        .map_err(|_| ApiError::new("sessionUnavailable", "Game session lock is poisoned"))?;
    *session = Some(game);
    Ok(NewGameResponse { snapshot, map })
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MovementOptionsRequest {
    unit_id: UnitId,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MovementOptionsResponse {
    revision: u64,
    unit_id: UnitId,
    modes: Vec<MovementModeOptions>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AttackTargetOptionsResponse {
    revision: u64,
    hex_ids: Vec<HexId>,
}

fn with_game<T>(
    state: &State<'_, AppState>,
    read: impl FnOnce(&GameState) -> Result<T, RuleError>,
) -> Result<T, ApiError> {
    let session = state
        .game
        .lock()
        .map_err(|_| ApiError::new("sessionUnavailable", "Game session lock is poisoned"))?;
    let game = session
        .as_ref()
        .ok_or_else(|| ApiError::new("gameNotStarted", "No game has been started"))?;
    Ok(read(game)?)
}

/// Read-only movement preview: availability and legal destinations for every
/// movement system of one unit.
#[tauri::command]
fn movement_options(
    request: MovementOptionsRequest,
    state: State<'_, AppState>,
) -> Result<MovementOptionsResponse, ApiError> {
    with_game(&state, |game| {
        Ok(MovementOptionsResponse {
            revision: game.snapshot().revision,
            modes: game.movement_modes(&request.unit_id)?,
            unit_id: request.unit_id,
        })
    })
}

/// Read-only preview of the hexes the planning side may mark as attack objectives.
#[tauri::command]
fn attack_target_options(state: State<'_, AppState>) -> Result<AttackTargetOptionsResponse, ApiError> {
    with_game(&state, |game| {
        Ok(AttackTargetOptionsResponse {
            revision: game.snapshot().revision,
            hex_ids: game.attack_target_options()?,
        })
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AirStrikeOptionsResponse {
    revision: u64,
    #[serde(flatten)]
    options: AirStrikeOptions,
}

/// Read-only preview of the phasing side's Air Strike Segment: Airspace for
/// Tactical Air Points and every targetable enemy unit with its modifier.
#[tauri::command]
fn air_strike_options(state: State<'_, AppState>) -> Result<AirStrikeOptionsResponse, ApiError> {
    with_game(&state, |game| {
        Ok(AirStrikeOptionsResponse {
            revision: game.snapshot().revision,
            options: game.air_strike_options()?,
        })
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombatOptionsResponse {
    revision: u64,
    #[serde(flatten)]
    options: CombatOptions,
}

/// Read-only preview of the Combat Phase: attackable hexes and eligible attackers.
#[tauri::command]
fn combat_options(state: State<'_, AppState>) -> Result<CombatOptionsResponse, ApiError> {
    with_game(&state, |game| {
        Ok(CombatOptionsResponse {
            revision: game.snapshot().revision,
            options: game.combat_options()?,
        })
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BattlePreviewRequest {
    hex_id: HexId,
    unit_ids: Vec<UnitId>,
    #[serde(default)]
    supporting_hq_id: Option<UnitId>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BattlePreviewResponse {
    revision: u64,
    odds: BattleOdds,
}

/// Read-only odds for a proposed attack; the battle itself is `resolveBattle`.
#[tauri::command]
fn battle_preview(
    request: BattlePreviewRequest,
    state: State<'_, AppState>,
) -> Result<BattlePreviewResponse, ApiError> {
    with_game(&state, |game| {
        Ok(BattlePreviewResponse {
            revision: game.snapshot().revision,
            odds: game.battle_preview(
                &request.hex_id,
                &request.unit_ids,
                request.supporting_hq_id.as_ref(),
            )?,
        })
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
            submit_game_command,
            movement_options,
            attack_target_options,
            air_strike_options,
            combat_options,
            battle_preview
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
