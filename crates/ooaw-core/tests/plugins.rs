//! Third-party plugins extending the official rules, using the example
//! night-fighting plugin built alongside the kernel.

use std::sync::{Arc, OnceLock};

use ooaw_core::{official_plugin, GameEngine, GameId, PluginModule, ScenarioRequest};
use serde_json::{json, Value};

const SOVIET: &str = "soviet.2gta.21motorRifleDivision";
const GERMAN: &str = "westGermany.6panzergrenadierDivision.16panzergrenadierBrigade";

fn night_fighting() -> Arc<PluginModule> {
    static PLUGIN: OnceLock<Arc<PluginModule>> = OnceLock::new();
    PLUGIN
        .get_or_init(|| {
            PluginModule::from_wasm(include_bytes!(concat!(
                env!("OUT_DIR"),
                "/ooaw_example_night_fighting.wasm"
            )))
            .unwrap()
        })
        .clone()
}

/// BALTAP's opening Warsaw Pact Combat Phase: one Soviet division next to a
/// West German brigade it has marked for attack.
fn combat_game(plugins: Vec<Arc<PluginModule>>) -> GameEngine {
    let setup = json!({
        "units": [{ "id": SOVIET, "hex": "4009" }, { "id": GERMAN, "hex": "4010" }],
        "attackTargets": ["4010"],
    });
    let mut game = GameEngine::with_plugins(
        GameId("plugins".to_owned()),
        plugins,
        ScenarioRequest::from("nato-baltap-1983"),
        Some(setup),
    )
    .unwrap();
    while game.snapshot().turn.current_step.unwrap().phase_id.0 != "combat" {
        game.execute(json!({ "type": "endPhase" })).unwrap();
    }
    game
}

fn preview(game: &mut GameEngine) -> Value {
    game.query(
        "battlePreview",
        json!({ "hexId": "4010", "unitIds": [SOVIET] }),
    )
    .unwrap()
}

#[test]
fn the_example_plugin_takes_part_in_one_filter_and_keeps_no_state() {
    let plugin = night_fighting();
    let manifest = plugin.manifest();
    assert_eq!(manifest.id, "example.nightFighting");
    assert!(manifest.stateless);
    assert_eq!(
        manifest.filters,
        vec!["nato.combat.columnShifts".to_owned()]
    );
    assert!(!plugin.is_trusted());
    assert!(official_plugin().unwrap().is_trusted());
}

#[test]
fn a_filter_plugin_shifts_official_battle_odds() {
    // 8 against 3 is 2:1; the turn-one Surprise makes it 3:1.
    let mut official_only = combat_game(vec![official_plugin().unwrap()]);
    assert_eq!(preview(&mut official_only)["finalOdds"], "3:1");

    let mut game = combat_game(vec![official_plugin().unwrap(), night_fighting()]);
    let odds = preview(&mut game);
    assert_eq!(odds["finalOdds"], "2:1");
    assert!(odds["shifts"]
        .as_array()
        .unwrap()
        .contains(&json!({ "reason": "example.nightFighting", "shift": -1 })));

    // The battle itself is fought at the filtered odds.
    let outcome = game
        .execute(json!({ "type": "resolveBattle", "hexId": "4010", "unitIds": [SOVIET] }))
        .unwrap();
    let report = outcome
        .events
        .iter()
        .find(|event| event["type"] == "battleResolved")
        .map(|event| &event["report"])
        .unwrap();
    assert_eq!(report["odds"]["finalOdds"], "2:1");
}

#[test]
fn a_game_needs_a_plugin_that_owns_its_scenario() {
    let error = GameEngine::with_plugins(
        GameId("plugins".to_owned()),
        vec![night_fighting()],
        ScenarioRequest::from("nato-baltap-1983"),
        None,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "scenarioNotFound");
}
