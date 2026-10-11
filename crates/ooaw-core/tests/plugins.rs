//! Loading and validating the example third-party plugin built alongside the kernel.

use std::sync::{Arc, OnceLock};

use ooaw_core::{official_plugin, GameEngine, GameId, PluginModule, ScenarioRequest};

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
fn a_game_rejects_more_than_one_plugin() {
    let error = GameEngine::with_plugins(
        GameId("plugins".to_owned()),
        vec![official_plugin().unwrap(), night_fighting()],
        ScenarioRequest::from("nato-baltap-1983"),
        None,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "pluginCount");
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
