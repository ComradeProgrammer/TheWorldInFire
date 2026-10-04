//! Line-oriented JSON adapter for driving the OOAW game kernel as a process.
//!
//! Each line on stdin is one request and produces exactly one response on
//! stdout. Diagnostics belong on stderr so stdout remains machine-readable.

use std::io::{self, BufRead, Write};

use ooaw_core::{GameEngine, GameId, GameSnapshot, MapDefinition, ScenarioSummary};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Requests accepted by the newline-delimited JSON process protocol.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum EngineRequest {
    ListScenarios,
    NewGame {
        /// Built-in scenario identifier used to create the game.
        scenario_id: String,
        /// Client-selected game identifier, also used to seed deterministic dice.
        game_id: String,
        /// Optional custom starting situation, in the scenario plugin's setup format.
        #[serde(default)]
        setup: Option<Value>,
    },
    GetSnapshot,
    SubmitCommand {
        /// Revision the client last observed; must match the active game before execution.
        expected_revision: u64,
        /// Command object, with a `type` tag, to validate and execute for the active game.
        command: Value,
    },
}

/// Responses written by the engine for each JSON request.
#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum EngineResponse {
    Scenarios {
        /// Metadata for the scenarios available to new-game requests.
        scenarios: Vec<ScenarioSummary>,
    },
    GameStarted {
        /// Authoritative game state at the time this response is produced.
        snapshot: GameSnapshot,
        /// Static scenario map supplied when the client starts a game.
        map: Box<MapDefinition>,
    },
    Snapshot {
        /// Authoritative game state at the time this response is produced.
        snapshot: GameSnapshot,
    },
    CommandAccepted {
        /// Authoritative revision after the accepted command.
        revision: u64,
        /// Ordered game events produced by the accepted command.
        events: Vec<Value>,
        /// Authoritative game state at the time this response is produced.
        snapshot: GameSnapshot,
    },
    Error {
        /// Stable error identifier for client-side handling.
        code: String,
        /// Human-readable explanation of the rejected request.
        message: String,
    },
}

impl EngineResponse {
    /// Builds a protocol error response from a stable code and a human-readable message.
    fn error(code: &str, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

/// Game state retained between requests in one engine process.
#[derive(Default)]
struct EngineSession {
    /// Active game for this process session, or none before a successful new-game request.
    game: Option<GameEngine>,
}

impl EngineSession {
    /// Dispatches a protocol request and checks command revisions against the active game.
    fn handle(&mut self, request: EngineRequest) -> EngineResponse {
        match request {
            EngineRequest::ListScenarios => match ooaw_core::list_scenarios() {
                Ok(scenarios) => EngineResponse::Scenarios { scenarios },
                Err(error) => EngineResponse::error(&error.code, error.message),
            },
            EngineRequest::NewGame {
                scenario_id,
                game_id,
                setup,
            } => {
                let scenario = scenario_id.as_str();
                let game = match setup {
                    Some(setup) => GameEngine::with_setup(GameId(game_id), scenario, setup),
                    None => GameEngine::new(GameId(game_id), scenario),
                };
                match game {
                    Ok(game) => {
                        let response = EngineResponse::GameStarted {
                            snapshot: game.snapshot(),
                            map: Box::new(game.map().clone()),
                        };
                        self.game = Some(game);
                        response
                    }
                    Err(error) => EngineResponse::error(&error.code, error.message),
                }
            }
            EngineRequest::GetSnapshot => match &self.game {
                Some(game) => EngineResponse::Snapshot {
                    snapshot: game.snapshot(),
                },
                None => EngineResponse::error("gameNotStarted", "No game has been started"),
            },
            EngineRequest::SubmitCommand {
                expected_revision,
                command,
            } => {
                let Some(game) = &mut self.game else {
                    return EngineResponse::error("gameNotStarted", "No game has been started");
                };
                let actual_revision = game.revision();
                if expected_revision != actual_revision {
                    return EngineResponse::error(
                        "revisionMismatch",
                        format!(
                            "Expected revision {expected_revision}, but the current revision is {actual_revision}"
                        ),
                    );
                }
                match game.execute(command) {
                    Ok(outcome) => EngineResponse::CommandAccepted {
                        revision: outcome.revision,
                        events: outcome.events,
                        snapshot: outcome.snapshot,
                    },
                    Err(error) => EngineResponse::error(&error.code, error.message),
                }
            }
        }
    }
}

/// Reads JSON requests from stdin and writes one flushed JSON response per line to stdout.
fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut session = EngineSession::default();

    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) => match serde_json::from_str::<EngineRequest>(&line) {
                Ok(request) => session.handle(request),
                Err(error) => EngineResponse::error("invalidRequest", error.to_string()),
            },
            Err(error) => {
                eprintln!("failed to read engine request: {error}");
                return Err(error);
            }
        };

        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }

    Ok(())
}
