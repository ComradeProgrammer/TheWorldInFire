//! Kernel behaviour with the bundled official rules plugin.

use ooaw_core::{official_plugin, GameEngine, GameId};
use serde_json::{json, Value};

fn new_game(scenario: &str) -> GameEngine {
    GameEngine::new(GameId("kernel-test".to_owned()), scenario).unwrap()
}

fn phase(game: &GameEngine) -> String {
    game.snapshot().turn.current_step.unwrap().phase_id.0
}

#[test]
fn the_official_plugin_describes_itself() {
    let plugin = official_plugin().unwrap();
    let manifest = plugin.manifest();
    assert_eq!(manifest.id, "ooaw.nato");
    assert!(manifest.phases.iter().any(|phase| phase == "combat"));
    assert!(manifest
        .commands
        .iter()
        .any(|command| command == "resolveBattle"));
    assert!(manifest
        .queries
        .iter()
        .any(|query| query == "battlePreview"));
    assert!(manifest
        .scenarios
        .iter()
        .any(|scenario| scenario.id == "nato-baltap-1983"));
}

#[test]
fn a_new_game_opens_on_the_first_interactive_phase() {
    let game = new_game("nato-1983-standard");
    let snapshot = game.snapshot();
    assert_eq!(snapshot.revision, 0);
    assert_eq!(phase(&game), "battlePlanning");
    assert_eq!(snapshot.units.len(), 2);

    let json = serde_json::to_value(&snapshot).unwrap();
    // Rules-owned state appears as top-level snapshot fields.
    assert!(json.get("battlePlan").is_some_and(Value::is_object));
    assert!(json.get("airPoints").is_some_and(Value::is_array));
    assert!(json.get("combat").is_some_and(Value::is_null));
    // Rules-owned unit markers appear as unit fields.
    assert!(json["units"][0].get("supply").is_some());
}

#[test]
fn end_phase_runs_through_automatic_phases() {
    let mut game = new_game("nato-1983-standard");
    let outcome = game.execute(json!({ "type": "endPhase" })).unwrap();
    assert_eq!(outcome.revision, 1);
    let types: Vec<&str> = outcome
        .events
        .iter()
        .filter_map(|event| event["type"].as_str())
        .collect();
    assert!(types.contains(&"phaseEnded"));
    assert!(types.contains(&"phaseStarted"));
}

#[test]
fn a_rejected_command_changes_nothing() {
    let mut game = new_game("nato-1983-standard");
    let before = game.snapshot();
    let dice = game.dice().clone();
    let error = game
        .execute(json!({ "type": "resolveBattle", "hexId": "2806", "unitIds": [] }))
        .unwrap_err();
    assert_eq!(error.code, "wrongPhase");
    assert_eq!(game.snapshot(), before);
    assert_eq!(game.dice(), &dice);
}

#[test]
fn unknown_and_debug_commands_are_refused() {
    let mut game = new_game("nato-1983-standard");
    assert_eq!(
        game.execute(json!({ "type": "launchMissiles" }))
            .unwrap_err()
            .code,
        "unknownCommand"
    );
    let check = json!({ "type": "debug.checkSupply", "sideId": "nato" });
    assert_eq!(
        game.execute(check.clone()).unwrap_err().code,
        "debugCommandsDisabled"
    );
    game.enable_debug_commands();
    game.execute(check).unwrap();
}

#[test]
fn queries_answer_without_changing_state() {
    let mut game = new_game("nato-1983-standard");
    let before = game.snapshot();
    let targets = game.query("attackTargetOptions", Value::Null).unwrap();
    assert!(targets.is_array());
    assert_eq!(
        game.query("noSuchQuery", Value::Null).unwrap_err().code,
        "unknownQuery"
    );
    assert_eq!(game.snapshot(), before);
}

#[test]
fn direct_state_edits_reach_the_plugin() {
    let mut game = new_game("nato-1983-standard");
    let unit_id = ooaw_core::api::UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());
    game.edit_state(|state| {
        state.units.remove(&unit_id);
    });
    let modes = game.query("movementModes", json!({ "unitId": unit_id }));
    assert!(modes.is_err());
}
