//! The six campaign scenarios, played through the kernel and the WebAssembly plugin.

mod common;

use common::TestGame;
use ooaw_core::api::GameStatus;
use ooaw_core::GameId;
use ooaw_nato::{find_scenario, GameCommand, ScenarioDefinition, UnitLocation};

const CAMPAIGNS: [(&str, usize, usize); 6] = [
    ("nato-strategic-surprise-1983", 199, 287),
    ("nato-strategic-surprise-1988", 202, 286),
    ("nato-extended-buildup-1983", 341, 362),
    ("nato-extended-buildup-1988", 339, 360),
    ("nato-war-of-nerves-1983", 199, 287),
    ("nato-war-of-nerves-1988", 202, 286),
];

#[test]
fn campaigns_deploy_receive_reinforcements_and_finish_fourteen_war_turns() {
    for (id, opening_count, final_count) in CAMPAIGNS {
        let scenario = find_scenario(id).unwrap();
        // Engine construction validates every ID, side, map hex, step and turn.
        let mut game = TestGame::new(GameId(id.to_owned()), scenario).unwrap();
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
    let unit = |scenario: &ScenarioDefinition, id: &str| {
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
                hex_id: ooaw_nato::HexId("2716".to_owned())
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

#[test]
fn only_the_published_campaign_years_exist() {
    assert!(find_scenario("nato-strategic-surprise-1990").is_none());
}
