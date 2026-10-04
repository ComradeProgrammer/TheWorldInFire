use std::collections::HashSet;
use std::io::Write;
use std::process::{Command, Stdio};

use ooaw_core::{find_scenario, GameCommand, GameEngine, GameId, GameStatus, UnitLocation};
use serde_json::{json, Value};

const CAMPAIGNS: [(&str, usize, usize); 6] = [
    ("nato-strategic-surprise-1983", 199, 287),
    ("nato-strategic-surprise-1988", 202, 286),
    ("nato-extended-buildup-1983", 341, 362),
    ("nato-extended-buildup-1988", 339, 360),
    ("nato-war-of-nerves-1983", 199, 287),
    ("nato-war-of-nerves-1988", 202, 286),
];

#[test]
fn json_clients_can_list_and_create_every_campaign() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ooaw-engine"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    writeln!(input, "{}", json!({"type": "listScenarios"})).unwrap();
    for (id, _, _) in CAMPAIGNS {
        writeln!(
            input,
            "{}",
            json!({"type": "newGame", "scenarioId": id, "gameId": id})
        )
        .unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 7);
    let summaries = responses[0]["scenarios"].as_array().unwrap();
    let ids: HashSet<_> = summaries
        .iter()
        .map(|summary| summary["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 8);
    assert!(ids.contains("nato-baltap-1983"));
    assert!(ids.contains("nato-1983-standard"));
    for (index, (id, _, _)) in CAMPAIGNS.iter().enumerate() {
        assert!(ids.contains(id));
        let response = &responses[index + 1];
        assert_eq!(response["type"], "gameStarted");
        assert_eq!(response["snapshot"]["scenario"]["id"], *id);
        assert_eq!(response["snapshot"]["scenario"]["maxGameTurns"], 14);
        assert_eq!(response["map"]["id"], "nato-central-europe");
    }
    assert!(find_scenario("nato-strategic-surprise-1990").is_none());
}

#[test]
fn campaigns_deploy_receive_reinforcements_and_finish_fourteen_war_turns() {
    for (id, opening_count, final_count) in CAMPAIGNS {
        let scenario = find_scenario(id).unwrap();
        // Engine construction validates every ID, side, map hex, step and turn.
        let mut game = GameEngine::new(GameId(id.to_owned()), scenario).unwrap();
        // The opening deployment is resolved when the game is created.
        let opening = game.snapshot();
        assert_eq!(opening.units.len(), opening_count, "{id}");
        assert!(opening.units.iter().any(|u| {
            u.definition.side_id.0 == "warsawPact" && matches!(u.location, UnitLocation::Hex { .. })
        }));
        assert!(opening.units.iter().any(|u| {
            u.definition.side_id.0 == "nato" && matches!(u.location, UnitLocation::Hex { .. })
        }));
        // Eight interactive phases per war turn: four for each side.
        for _ in 0..14 * 8 {
            game.execute(GameCommand::EndPhase).unwrap();
        }
        let finished = game.snapshot();
        assert_eq!(finished.status, GameStatus::Completed, "{id}");
        assert_eq!(finished.units.len(), final_count, "{id}");
        assert!(finished.turn.current_step.is_none());
    }
}

#[test]
fn years_use_their_own_counter_values_and_soviet_front_assignments() {
    let first = find_scenario("nato-strategic-surprise-1983").unwrap();
    let second = find_scenario("nato-strategic-surprise-1988").unwrap();
    let unit = |scenario: &ooaw_core::ScenarioDefinition, id: &str| {
        scenario
            .reinforcements
            .iter()
            .find(|u| u.unit.id().0 == id)
            .unwrap()
            .clone()
    };
    // Play Booklet 44.3/44.4: the same British brigade upgrades in 1988.
    assert_eq!(
        unit(&first, "unitedKingdom.4.3a")
            .unit
            .current_step()
            .unwrap()
            .attack,
        4
    );
    assert_eq!(
        unit(&second, "unitedKingdom.4.3a")
            .unit
            .current_step()
            .unwrap()
            .attack,
        5
    );
    assert_eq!(
        unit(&first, "unitedKingdom.22.1a")
            .unit
            .definition
            .unit_type_id
            .0,
        "mechanizedBrigade"
    );
    assert_eq!(
        unit(&second, "unitedKingdom.22.1a")
            .unit
            .definition
            .unit_type_id
            .0,
        "tankBrigade"
    );
    // 44.2: 6G and 90GT exchange fronts between the two years.
    assert_eq!(unit(&first, "sovietUnion.6gdivision").game_turn, 1);
    assert_eq!(unit(&second, "sovietUnion.6gdivision").game_turn, 9);
    assert_eq!(unit(&first, "sovietUnion.90gt").game_turn, 9);
    assert_eq!(unit(&second, "sovietUnion.90gt").game_turn, 1);
    for scenario in [&first, &second] {
        // Living erratum 37.3: West German 9/3Pz starts at 2716, not 3223.
        assert_eq!(
            unit(scenario, "westGermany.9.3pz").unit.location,
            UnitLocation::Hex {
                hex_id: ooaw_core::HexId("2716".to_owned())
            }
        );
    }
}

#[test]
fn campaign_support_and_airlift_follow_the_scenario_instructions() {
    for (id, _, _) in CAMPAIGNS {
        let scenario = find_scenario(id).unwrap();
        let extended = id.contains("extended-buildup");
        let rules = &scenario.battle_planning_rules;
        assert_eq!(
            rules.warsaw_pact_airlift_commands,
            if extended { 4 } else { 3 }
        );
        assert_eq!(rules.nato_airlift_commands, if extended { 3 } else { 2 });
        assert_eq!(
            rules.air_power.surprise_turn,
            id.contains("strategic-surprise").then_some(1)
        );
        for hq in ["sovietUnion.baf", "unitedStates.usiii"] {
            assert_eq!(
                scenario
                    .offensive_support_hqs
                    .iter()
                    .any(|h| h.hq_id.0 == hq),
                extended
            );
        }
        assert!(!scenario
            .offensive_support_hqs
            .iter()
            .any(|h| h.hq_id.0 == "sovietUnion.bc"));
        assert!(scenario.offensive_support_hqs.iter().all(|hq| {
            scenario
                .reinforcements
                .iter()
                .any(|u| u.unit.id() == &hq.hq_id)
                && hq.formations.iter().all(|f| {
                    scenario
                        .reinforcements
                        .iter()
                        .any(|u| u.unit.definition.formation_id.as_ref() == Some(f))
                })
        }));
    }
}
