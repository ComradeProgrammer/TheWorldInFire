use crate::dice::Dice;
use crate::model::{
    find_scenario, list_scenarios, AirPointKind, AirPointSource, Airspace, Disruption, HexId,
    MovementMode, PhaseActor, PhaseExecution, SideId, StrikeResult, SupplyStatus, TrainStatus,
    UnitId, UnitLocation, UnitState,
};
use crate::{GameCommand, GameEngine, GameEvent, GameId, GameStatus};

/// Creates a fresh standard NATO scenario engine for unit tests.
fn new_game() -> GameEngine {
    GameEngine::new(
        GameId("test-game".to_owned()),
        find_scenario("nato-1983-standard").unwrap(),
    )
    .unwrap()
}

/// Verifies a new game opens at turn one, revision zero, on the first step that needs a player.
#[test]
fn new_game_starts_at_the_first_step_that_needs_a_player() {
    let game = new_game();
    assert_eq!(game.map().id, "nato-central-europe");
    assert_eq!(game.map().version, 1);
    assert_eq!(game.map().hexes.len(), 1_458);
    assert!(game.map().hexes.iter().any(|hex| {
        hex.id.0 == "1310"
            && hex
                .city
                .as_ref()
                .is_some_and(|city| city.name == "København")
    }));
    let snapshot = game.snapshot();
    let step = snapshot.turn.current_step.unwrap();

    assert_eq!(snapshot.revision, 0);
    assert_eq!(snapshot.protocol_version, 17);
    assert_eq!(snapshot.turn.game_turn, 1);
    // Joint Status, Joint Reinforcement, and Pre-Battle are automatic.
    assert_eq!(step.phase_id.0, "battlePlanning");
    assert_eq!(
        step.actor,
        PhaseActor::Side {
            side_id: SideId("warsawPact".to_owned())
        }
    );
    assert_eq!(snapshot.scenario.map_id, "nato-central-europe");
    assert_eq!(snapshot.units.len(), 2);
}

/// Verifies snapshots identify the scenario map without duplicating its static data.
#[test]
fn snapshot_identifies_the_scenario_map_without_repeating_its_data() {
    let json = serde_json::to_value(new_game().snapshot()).unwrap();

    assert_eq!(json["protocolVersion"], 17);
    assert_eq!(json["scenario"]["mapId"], "nato-central-europe");
    assert!(json.get("map").is_none());
}

/// Verifies new-game map serialization includes grid coordinates, hexes, and hexside features.
#[test]
fn scenario_map_serializes_for_the_new_game_bootstrap() {
    let game = new_game();
    let json = serde_json::to_value(game.map()).unwrap();

    assert_eq!(json["id"], "nato-central-europe");
    assert_eq!(json["grid"]["columnBase"], 38);
    assert_eq!(json["hexes"][0]["id"], "0114");
    assert!(json["hexsides"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
}

/// Verifies scenario validation rejects reinforcement units deployed to unknown map hexes.
#[test]
fn scenario_validation_rejects_a_unit_outside_its_map() {
    let mut scenario = find_scenario("nato-1983-standard").unwrap();
    scenario.reinforcements[0].unit.location = UnitLocation::Hex {
        hex_id: crate::HexId("9999".to_owned()),
    };

    let error = GameEngine::new(GameId("invalid-map-unit".to_owned()), scenario)
        .err()
        .expect("unknown deployment hex should be rejected");

    assert_eq!(error.code, "invalidScenario");
    assert!(error.message.contains("unknown map hex"));
}

/// Verifies scenario validation rejects hexsides with unknown map endpoints.
#[test]
fn scenario_validation_rejects_a_hexside_outside_its_map() {
    let mut scenario = find_scenario("nato-1983-standard").unwrap();
    scenario.map.hexsides[0].a = crate::HexId("9999".to_owned());

    let error = GameEngine::new(GameId("invalid-map-hexside".to_owned()), scenario)
        .err()
        .expect("unknown hexside endpoint should be rejected");

    assert_eq!(error.code, "invalidScenario");
    assert!(error.message.contains("unknown hex"));
}

/// Verifies the opening deployment is resolved before the first player decision.
#[test]
fn opening_units_are_deployed_when_the_game_is_created() {
    let game = new_game();
    let snapshot = game.snapshot();

    assert_eq!(snapshot.revision, 0);
    assert_eq!(snapshot.units.len(), 2);
    assert_eq!(
        snapshot.turn.current_step.unwrap().phase_id.0,
        "battlePlanning"
    );
    let division = snapshot
        .units
        .iter()
        .find(|unit| unit.id().0 == "soviet.6thGuardsMotorRifleDivision")
        .expect("opening WP division");
    assert_eq!(division.current_step().unwrap().attack, 8);
    assert_eq!(division.definition.steps.len(), 2);
    assert_eq!(
        division.location,
        UnitLocation::Hex {
            hex_id: HexId("2806".to_owned())
        }
    );
}

/// Verifies post-battle cleanup and the next side's pre-battle supply run automatically.
#[test]
fn automatic_post_battle_is_processed_without_user_input() {
    let mut game = new_game();
    for _ in 0..3 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    assert_eq!(
        game.snapshot().turn.current_step.unwrap().phase_id.0,
        "reserve"
    );

    let outcome = game.execute(GameCommand::EndPhase).unwrap();
    let snapshot = outcome.snapshot;
    let step = snapshot.turn.current_step.unwrap();

    assert_eq!(step.phase_id.0, "battlePlanning");
    assert_eq!(
        step.actor,
        PhaseActor::Side {
            side_id: SideId("nato".to_owned())
        }
    );
    assert!(outcome.events.iter().any(|event| matches!(
        event,
        GameEvent::PhaseEnded { step, .. } if step.phase_id.0 == "postBattle"
    )));
    assert!(outcome.events.iter().any(|event| matches!(
        event,
        GameEvent::PreBattleSupplyChecked { side_id, .. } if side_id.0 == "nato"
    )));
}

/// Verifies both sides' pre-battle steps are defined as automatic phases.
#[test]
fn pre_battle_is_automatic_for_both_sides() {
    let scenario = find_scenario("nato-1983-standard").unwrap();
    let pre_battle_steps: Vec<_> = scenario
        .turn_sequence
        .iter()
        .filter(|step| step.phase_id.0 == "preBattle")
        .collect();

    assert_eq!(pre_battle_steps.len(), 2);
    assert!(pre_battle_steps
        .iter()
        .all(|step| step.execution == PhaseExecution::Automatic));
}

/// Verifies scenario data assigns a Battle Planning Phase to each side.
#[test]
fn sequence_is_data_driven_for_both_sides() {
    let scenario = find_scenario("nato-1983-standard").unwrap();
    let planning_actors: Vec<_> = scenario
        .turn_sequence
        .iter()
        .filter(|step| step.phase_id.0 == "battlePlanning")
        .map(|step| step.actor.clone())
        .collect();

    assert_eq!(planning_actors.len(), 2);
    assert!(planning_actors.contains(&PhaseActor::Side {
        side_id: SideId("warsawPact".to_owned())
    }));
    assert!(planning_actors.contains(&PhaseActor::Side {
        side_id: SideId("nato".to_owned())
    }));
}

/// Verifies joint status and reinforcement phases precede both player turns.
#[test]
fn joint_phases_precede_both_side_turns() {
    let scenario = find_scenario("nato-1983-standard").unwrap();

    assert_eq!(scenario.turn_sequence[0].phase_id.0, "jointStatus");
    assert_eq!(scenario.turn_sequence[0].actor, PhaseActor::All);
    assert_eq!(scenario.turn_sequence[1].phase_id.0, "jointReinforcement");
    assert_eq!(scenario.turn_sequence[1].actor, PhaseActor::All);
    assert_eq!(scenario.turn_sequence[2].phase_id.0, "preBattle");
}

/// Verifies the game completes after the final interactive phase and automatic cleanup.
#[test]
fn final_turn_completes_after_last_interactive_phase_and_cleanup() {
    let mut game = new_game();
    let interactive_steps_per_turn = 8;
    for _ in 0..(14 * interactive_steps_per_turn) {
        game.execute(GameCommand::EndPhase).unwrap();
    }

    assert_eq!(game.snapshot().status, GameStatus::Completed);
    assert!(game.snapshot().turn.current_step.is_none());
}

/// Verifies a scheduled reinforcement stays absent until its designated game turn.
#[test]
fn reinforcements_are_added_only_on_their_scheduled_turn() {
    let mut scenario = find_scenario("nato-1983-standard").unwrap();
    scenario.reinforcements[0].game_turn = 2;
    scenario.reinforcements.truncate(1);
    let unit_id = scenario.reinforcements[0].unit.id().clone();
    let mut game = GameEngine::new(GameId("scheduled-game".to_owned()), scenario).unwrap();

    assert!(game.snapshot().units.is_empty());

    for _ in 0..7 {
        game.execute(GameCommand::EndPhase).unwrap();
    }

    let snapshot = game.execute(GameCommand::EndPhase).unwrap().snapshot;
    assert_eq!(snapshot.turn.game_turn, 2);
    assert_eq!(snapshot.units.len(), 1);
    assert_eq!(snapshot.units[0].id(), &unit_id);
}

/// Verifies reinforcement events serialize their type and fields using the camelCase IPC contract.
#[test]
fn reinforcement_event_serializes_as_a_camel_case_ipc_message() {
    let unit = find_scenario("nato-1983-standard").unwrap().reinforcements[0]
        .unit
        .clone();
    let event = GameEvent::ReinforcementsArrived {
        game_turn: 1,
        units: vec![unit],
    };

    let json = serde_json::to_value(event).unwrap();

    assert_eq!(json["type"], "reinforcementsArrived");
    assert_eq!(json["gameTurn"], 1);
    assert_eq!(json["units"][0]["sideId"], "warsawPact");
    assert_eq!(json["units"][0]["strengthStepIndex"], 0);
    assert_eq!(json["units"][0]["steps"][0]["attack"], 8);
    assert_eq!(json["units"][0]["steps"][0]["defense"], 6);
    assert_eq!(json["units"][0]["steps"][0]["movement"], 5);
    assert!(json["units"][0]["steps"][0].get("counterAssetId").is_none());
    assert_eq!(json["units"][0]["location"]["type"], "hex");
    assert_eq!(json["units"][0]["location"]["hexId"], "2806");
    assert_eq!(
        json["units"][0]["supply"]["headquarters"],
        serde_json::Value::Null
    );
    assert_eq!(json["units"][0]["supply"]["movement"], "supplied");
    assert_eq!(json["units"][0]["supply"]["combat"], "supplied");
    assert!(json.get("game_turn").is_none());
}

/// Verifies supply-check events serialize unit identifiers and applicable supply state.
#[test]
fn pre_battle_supply_event_serializes_checked_unit_state() {
    let mut game = new_game();
    // Strike, Combat, and Reserve; the last ends the WP turn and runs NATO's Pre-Battle.
    for _ in 0..3 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    let outcome = game.execute(GameCommand::EndPhase).unwrap();
    let event = outcome
        .events
        .iter()
        .find(|event| matches!(event, GameEvent::PreBattleSupplyChecked { .. }))
        .expect("pre-battle supply event");

    let json = serde_json::to_value(event).unwrap();

    assert_eq!(json["type"], "preBattleSupplyChecked");
    assert_eq!(json["sideId"], "nato");
    assert_eq!(
        json["units"][0]["unitId"],
        "westGermany.1stBrigade.1stPanzerDivision"
    );
    assert_eq!(json["units"][0]["supply"]["movement"], "supplied");
}

/// Verifies BALTAP is registered with seven turns and the expected reinforcement schedule.
#[test]
fn baltap_is_available_with_its_seven_turn_unit_schedule() {
    let summaries = list_scenarios();
    let scenario = find_scenario("nato-baltap-1983").unwrap();

    assert!(summaries
        .iter()
        .any(|summary| summary.id == "nato-baltap-1983"));
    assert_eq!(scenario.max_game_turns, 7);
    assert_eq!(scenario.reinforcements.len(), 44);
    assert_eq!(
        scenario
            .reinforcements
            .iter()
            .filter(|reinforcement| reinforcement.game_turn == 1)
            .count(),
        27
    );
    assert_eq!(
        scenario
            .reinforcements
            .iter()
            .filter(|reinforcement| reinforcement.game_turn == 2)
            .count(),
        11
    );
}

/// Verifies BALTAP opening deployments and later arrivals both use joint reinforcement.
#[test]
fn baltap_opening_setup_and_later_reinforcements_use_the_same_phase() {
    let scenario = find_scenario("nato-baltap-1983").unwrap();
    let mut game = GameEngine::new(GameId("baltap-game".to_owned()), scenario).unwrap();

    // The opening deployment is resolved when the game is created.
    let opening = game.snapshot();
    assert_eq!(opening.units.len(), 27);
    assert_eq!(
        opening
            .units
            .iter()
            .filter(|unit| matches!(unit.location, UnitLocation::Hex { .. }))
            .count(),
        17
    );
    assert_eq!(
        opening
            .units
            .iter()
            .filter(|unit| matches!(unit.location, UnitLocation::StrategicReserve))
            .count(),
        10
    );

    for _ in 0..7 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    let turn_two = game.execute(GameCommand::EndPhase).unwrap();
    let arrivals = turn_two.events.iter().find_map(|event| match event {
        GameEvent::ReinforcementsArrived { game_turn, units } => Some((game_turn, units)),
        _ => None,
    });

    let (game_turn, units) = arrivals.expect("turn-two BALTAP reinforcements");
    assert_eq!(*game_turn, 2);
    assert_eq!(units.len(), 11);
    assert_eq!(turn_two.snapshot.units.len(), 38);
}

/// Verifies BALTAP completes after seven turns and applies its scheduled HQ withdrawal.
#[test]
fn baltap_ends_after_seven_turns() {
    let scenario = find_scenario("nato-baltap-1983").unwrap();
    let mut game = GameEngine::new(GameId("baltap-completion".to_owned()), scenario).unwrap();

    for _ in 0..(7 * 8) {
        game.execute(GameCommand::EndPhase).unwrap();
    }

    assert_eq!(game.snapshot().status, GameStatus::Completed);
    // All 44 units arrive; the NEGF HQ is withdrawn at the start of turn 4 (36.4.2.4).
    let units = game.snapshot().units;
    assert_eq!(units.len(), 43);
    assert!(units
        .iter()
        .all(|unit| unit.id().0 != "soviet.northernEastGermanyFront.hq"));
}

/// Verifies BALTAP supply checks distinguish HQ and combat supply and supply off-map reserves.
#[test]
fn baltap_pre_battle_records_applicable_supply_types() {
    let scenario = find_scenario("nato-baltap-1983").unwrap();
    let mut game = GameEngine::new(GameId("baltap-supply".to_owned()), scenario).unwrap();

    let outcome = game.execute(GameCommand::EndPhase).unwrap();
    let snapshot = outcome.snapshot;
    let hq = snapshot
        .units
        .iter()
        .find(|unit| unit.id().0 == "soviet.northernEastGermanyFront.hq")
        .expect("Warsaw Pact HQ");
    let reserve_unit = snapshot
        .units
        .iter()
        .find(|unit| unit.id().0 == "soviet.7guardsAirborneDivision.119regiment")
        .expect("Warsaw Pact strategic reserve unit");

    assert_eq!(hq.supply.headquarters, Some(SupplyStatus::Supplied));
    assert_eq!(hq.supply.movement, None);
    assert_eq!(hq.supply.combat, None);
    assert_eq!(reserve_unit.supply.headquarters, None);
    assert_eq!(reserve_unit.supply.movement, Some(SupplyStatus::Supplied));
    assert_eq!(reserve_unit.supply.combat, Some(SupplyStatus::Supplied));
}

/// Verifies planning accepts movement, resupply, and attack selections without a fixed action order.
#[test]
fn battle_plan_actions_can_be_submitted_in_any_order() {
    let mut game = new_game();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());

    game.execute(GameCommand::SetAttackTarget {
        hex_id: HexId("3216".to_owned()),
        selected: true,
    })
    .unwrap();
    game.execute(GameCommand::SetResupplyTarget {
        unit_id: unit_id.clone(),
        selected: true,
    })
    .unwrap();
    let moved = game
        .execute(GameCommand::MoveUnit {
            unit_id: unit_id.clone(),
            destination: HexId("2807".to_owned()),
            mode: MovementMode::Tactical,
        })
        .unwrap();

    let plan = moved.snapshot.battle_plan.expect("active plan");
    assert_eq!(plan.resupply_target_unit_ids, vec![unit_id]);
    assert_eq!(plan.attack_targets, vec![HexId("3216".to_owned())]);
    assert_eq!(plan.movements.len(), 1);
    assert_eq!(plan.movements[0].cost, 1);
    assert!(matches!(
        moved.events.as_slice(),
        [GameEvent::UnitMoved {
            mode: MovementMode::Tactical,
            ..
        }]
    ));
}

/// Verifies selected stacks receive resupply when their Battle Planning Phase ends.
#[test]
fn resupply_is_applied_to_the_selected_stack_when_planning_ends() {
    let mut game = new_game();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());
    let unit = game.units.get_mut(&unit_id).unwrap();
    unit.supply.movement = Some(SupplyStatus::OutOfSupply);
    unit.supply.combat = Some(SupplyStatus::OutOfSupply);

    game.execute(GameCommand::SetResupplyTarget {
        unit_id: unit_id.clone(),
        selected: true,
    })
    .unwrap();
    let outcome = game.execute(GameCommand::EndPhase).unwrap();
    let unit = outcome
        .snapshot
        .units
        .iter()
        .find(|unit| unit.id() == &unit_id)
        .unwrap();

    assert_eq!(unit.supply.movement, Some(SupplyStatus::Supplied));
    assert_eq!(unit.supply.combat, Some(SupplyStatus::Supplied));
    assert!(matches!(
        outcome.events.first(),
        Some(GameEvent::UnitsResupplied { unit_ids, .. }) if unit_ids == &vec![unit_id]
    ));
}

/// Verifies increased scenario resupply capacity supports multiple selected stacks.
#[test]
fn multiple_resupply_operations_are_applied_up_to_the_scenario_limit() {
    let first = UnitId(SOVIET.to_owned());
    let second = UnitId(SOVIET_2.to_owned());
    let mut game = baltap_planning("warsawPact", &[(SOVIET, "2412"), (SOVIET_2, "2413")]);
    game.scenario
        .battle_planning_rules
        .resupply_operations_per_turn = 2;
    for unit_id in [&first, &second] {
        let unit = game.units.get_mut(unit_id).unwrap();
        unit.supply.movement = Some(SupplyStatus::OutOfSupply);
        unit.supply.combat = Some(SupplyStatus::OutOfSupply);
        game.execute(GameCommand::SetResupplyTarget {
            unit_id: unit_id.clone(),
            selected: true,
        })
        .unwrap();
    }

    let outcome = game.execute(GameCommand::EndPhase).unwrap();

    assert_eq!(
        outcome
            .events
            .iter()
            .filter(|event| matches!(event, GameEvent::UnitsResupplied { .. }))
            .count(),
        2
    );
    for unit_id in [&first, &second] {
        let unit = outcome
            .snapshot
            .units
            .iter()
            .find(|unit| unit.id() == unit_id)
            .unwrap();
        assert_eq!(unit.supply.movement, Some(SupplyStatus::Supplied));
        assert_eq!(unit.supply.combat, Some(SupplyStatus::Supplied));
    }
}

/// Verifies excess resupply selections are rejected without advancing the revision.
#[test]
fn resupply_targets_cannot_exceed_the_scenario_limit() {
    let mut game = baltap_planning("warsawPact", &[(SOVIET, "2412"), (SOVIET_2, "2413")]);
    game.execute(GameCommand::SetResupplyTarget {
        unit_id: UnitId(SOVIET.to_owned()),
        selected: true,
    })
    .unwrap();
    let revision = game.snapshot().revision;

    let error = game
        .execute(GameCommand::SetResupplyTarget {
            unit_id: UnitId(SOVIET_2.to_owned()),
            selected: true,
        })
        .unwrap_err();

    assert_eq!(error.code, "resupplyLimitReached");
    assert_eq!(game.snapshot().revision, revision);
}

/// Verifies train loading completes on a later player turn before rail movement is allowed.
#[test]
fn entraining_takes_one_player_turn_before_rail_movement_is_available() {
    let mut game = new_game();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());

    let outcome = game
        .execute(GameCommand::EntrainUnit {
            unit_id: unit_id.clone(),
        })
        .unwrap();
    assert_eq!(
        outcome
            .snapshot
            .units
            .iter()
            .find(|unit| unit.id() == &unit_id)
            .unwrap()
            .train_status,
        Some(TrainStatus::Entraining)
    );
    assert_eq!(
        game.execute(GameCommand::MoveUnit {
            unit_id: unit_id.clone(),
            destination: HexId("2807".to_owned()),
            mode: MovementMode::Rail,
        })
        .unwrap_err()
        .code,
        "unitNotEntrained"
    );

    for _ in 0..8 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    let unit = game
        .snapshot()
        .units
        .into_iter()
        .find(|unit| unit.id() == &unit_id)
        .unwrap();
    assert_eq!(unit.train_status, Some(TrainStatus::Entrained));
}

/// Verifies undoing a detrain order restores its train marker and clears the plan's detrain record.
#[test]
fn a_detrain_order_can_be_undone_during_the_same_plan() {
    let mut game = new_game();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());
    game.execute(GameCommand::EntrainUnit {
        unit_id: unit_id.clone(),
    })
    .unwrap();
    for _ in 0..8 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    let train_status = |game: &GameEngine| {
        game.snapshot()
            .units
            .into_iter()
            .find(|unit| unit.id() == &unit_id)
            .unwrap()
            .train_status
    };
    assert_eq!(
        game.execute(GameCommand::UndoDetrainUnit {
            unit_id: unit_id.clone(),
        })
        .unwrap_err()
        .code,
        "unitNotDetrainedThisPlan"
    );

    game.execute(GameCommand::DetrainUnit {
        unit_id: unit_id.clone(),
    })
    .unwrap();
    assert_eq!(train_status(&game), None);
    assert!(game
        .snapshot()
        .battle_plan
        .unwrap()
        .detrained_unit_ids
        .contains(&unit_id));

    let outcome = game
        .execute(GameCommand::UndoDetrainUnit {
            unit_id: unit_id.clone(),
        })
        .unwrap();
    assert_eq!(
        outcome.events,
        vec![GameEvent::TrainStatusChanged {
            unit_id: unit_id.clone(),
            status: Some(TrainStatus::Entrained),
        }]
    );
    assert_eq!(train_status(&game), Some(TrainStatus::Entrained));
    assert!(outcome
        .snapshot
        .battle_plan
        .unwrap()
        .detrained_unit_ids
        .is_empty());
}

/// Verifies an airborne strategic reserve unit can airlift within scenario capacity.
#[test]
fn airborne_reserve_unit_can_use_scenario_limited_air_transport() {
    let scenario = find_scenario("nato-baltap-1983").unwrap();
    let mut game = GameEngine::new(GameId("airlift-game".to_owned()), scenario).unwrap();
    let unit_id = UnitId("soviet.7guardsAirborneDivision.119regiment".to_owned());

    let outcome = game
        .execute(GameCommand::MoveUnit {
            unit_id: unit_id.clone(),
            // House rule: air transport flies city to city (Rostock).
            destination: HexId("2111".to_owned()),
            mode: MovementMode::AirTransport,
        })
        .unwrap();

    assert!(matches!(
        outcome
            .snapshot
            .units
            .iter()
            .find(|unit| unit.id() == &unit_id)
            .unwrap()
            .location,
        UnitLocation::Hex { ref hex_id } if hex_id.0 == "2111"
    ));
    assert_eq!(outcome.snapshot.battle_plan.unwrap().airlift_steps_used, 1);
}

/// Verifies planning commands fail in other phases without advancing the revision.
#[test]
fn planning_commands_are_rejected_outside_battle_planning() {
    let mut game = new_game();
    game.execute(GameCommand::EndPhase).unwrap();
    let error = game
        .execute(GameCommand::SetAttackTarget {
            hex_id: HexId("3216".to_owned()),
            selected: true,
        })
        .unwrap_err();
    assert_eq!(error.code, "wrongPhase");
    assert_eq!(game.snapshot().revision, 1);
}

/// Verifies a deselected resupply unit is removed from the plan and emits a cancellation event.
#[test]
fn resupply_selection_can_be_cancelled() {
    let mut game = new_game();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());
    game.execute(GameCommand::SetResupplyTarget {
        unit_id: unit_id.clone(),
        selected: true,
    })
    .unwrap();

    let outcome = game
        .execute(GameCommand::SetResupplyTarget {
            unit_id: unit_id.clone(),
            selected: false,
        })
        .unwrap();

    assert_eq!(
        outcome
            .snapshot
            .battle_plan
            .unwrap()
            .resupply_target_unit_ids,
        Vec::<UnitId>::new()
    );
    assert!(matches!(
        outcome.events.as_slice(),
        [GameEvent::ResupplyTargetSet {
            unit_id: event_unit_id,
            selected: false,
            ..
        }] if event_unit_id == &unit_id
    ));
}

/// Verifies undo restores a unit's previous location and removes its last movement order.
#[test]
fn a_units_last_movement_can_be_undone() {
    let mut game = new_game();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());
    game.execute(GameCommand::MoveUnit {
        unit_id: unit_id.clone(),
        destination: HexId("2807".to_owned()),
        mode: MovementMode::Tactical,
    })
    .unwrap();

    let outcome = game
        .execute(GameCommand::UndoUnitMovement {
            unit_id: unit_id.clone(),
        })
        .unwrap();
    let unit = outcome
        .snapshot
        .units
        .iter()
        .find(|unit| unit.id() == &unit_id)
        .unwrap();

    assert!(matches!(&unit.location, UnitLocation::Hex { hex_id } if hex_id.0 == "2806"));
    assert!(outcome.snapshot.battle_plan.unwrap().movements.is_empty());
    assert!(matches!(
        outcome.events.as_slice(),
        [GameEvent::UnitMovementUndone { .. }]
    ));
}

/// Verifies planning and movement commands deserialize from camelCase IPC messages.
#[test]
fn commands_deserialize_from_camel_case_ipc_messages() {
    let command: GameCommand = serde_json::from_value(serde_json::json!({
        "type": "moveUnit",
        "unitId": "soviet.6thGuardsMotorRifleDivision",
        "destination": "2807",
        "mode": "tactical",
    }))
    .unwrap();
    assert_eq!(
        command,
        GameCommand::MoveUnit {
            unit_id: UnitId("soviet.6thGuardsMotorRifleDivision".to_owned()),
            destination: HexId("2807".to_owned()),
            mode: MovementMode::Tactical,
        }
    );
    let command: GameCommand = serde_json::from_value(serde_json::json!({
        "type": "setAttackTarget",
        "hexId": "3216",
        "selected": true,
    }))
    .unwrap();
    assert_eq!(
        command,
        GameCommand::SetAttackTarget {
            hex_id: HexId("3216".to_owned()),
            selected: true,
        }
    );
}

/// Starts BALTAP at the requested side's Battle Planning Phase with custom unit placements.
fn baltap_planning(side: &str, placements: &[(&str, &str)]) -> GameEngine {
    let mut game = GameEngine::new(
        GameId("movement-game".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
    )
    .unwrap();
    loop {
        let step = game.snapshot().turn.current_step.unwrap();
        if step.phase_id.0 == "battlePlanning"
            && matches!(&step.actor, PhaseActor::Side { side_id } if side_id.0 == side)
        {
            break;
        }
        game.execute(GameCommand::EndPhase).unwrap();
    }
    game.units
        .retain(|id, _| placements.iter().any(|(unit, _)| id.0 == *unit));
    for (unit, hex) in placements {
        game.units
            .get_mut(&UnitId((*unit).to_owned()))
            .unwrap()
            .location = UnitLocation::Hex {
            hex_id: HexId((*hex).to_owned()),
        };
    }
    game
}

/// Builds a Tactical movement command for the specified unit and destination hex.
fn move_to(unit: &str, hex: &str) -> GameCommand {
    GameCommand::MoveUnit {
        unit_id: UnitId(unit.to_owned()),
        destination: HexId(hex.to_owned()),
        mode: MovementMode::Tactical,
    }
}

const DANE: &str = "denmark.landzealand.1mechanizedBrigade";
const DANE_2: &str = "denmark.landzealand.2mechanizedBrigade";
const SOVIET: &str = "soviet.2gta.21motorRifleDivision";
const SOVIET_2: &str = "soviet.2gta.16guardsTankDivision";
const GERMAN: &str = "westGermany.6panzergrenadierDivision.16panzergrenadierBrigade";

/// Verifies an All-Sea hexside without a causeway blocks ground movement.
#[test]
fn ground_movement_cannot_cross_an_all_sea_hexside() {
    let mut game = baltap_planning("nato", &[(DANE, "1414")]);
    let options = game
        .movement_options(&UnitId(DANE.to_owned()), MovementMode::Tactical)
        .unwrap();
    assert!(options.iter().any(|option| option.hex_id.0 == "1514"));
    assert!(options.iter().all(|option| option.hex_id.0 != "1413"));
    assert_eq!(
        game.execute(move_to(DANE, "1413")).unwrap_err().code,
        "noLegalRoute"
    );
}

/// Verifies the Danish Ferry carries one NATO unit per direction and ends its movement.
#[test]
fn one_nato_unit_per_direction_may_cross_the_danish_ferry_and_must_stop() {
    let mut game = baltap_planning("nato", &[(DANE, "1514"), (DANE_2, "1514")]);
    let outcome = game.execute(move_to(DANE, "1513")).unwrap();
    let movement = &outcome.snapshot.battle_plan.unwrap().movements[0];
    assert_eq!(movement.path, vec![HexId("1513".to_owned())]);
    assert!(game
        .movement_options(&UnitId(DANE.to_owned()), MovementMode::Tactical)
        .unwrap()
        .is_empty());
    assert!(game
        .movement_options(&UnitId(DANE_2.to_owned()), MovementMode::Tactical)
        .unwrap()
        .iter()
        .all(|option| option.hex_id.0 != "1513"));
    assert_eq!(
        game.execute(move_to(DANE_2, "1513")).unwrap_err().code,
        "noLegalRoute"
    );

    let mut game = baltap_planning("warsawPact", &[(SOVIET, "1514")]);
    assert_eq!(
        game.execute(move_to(SOVIET, "1513")).unwrap_err().code,
        "noLegalRoute"
    );
}

/// Verifies a Major River crossing adds one point to the destination's entry cost.
#[test]
fn crossing_a_major_river_costs_one_extra_movement_point() {
    let game = baltap_planning("nato", &[(GERMAN, "4104")]);
    let river_crossing = game
        .movement_options(&UnitId(GERMAN.to_owned()), MovementMode::Tactical)
        .unwrap()
        .into_iter()
        .find(|option| option.hex_id.0 == "4103")
        .unwrap();
    assert_eq!(river_crossing.cost, 2);
    assert_eq!(river_crossing.path, vec![HexId("4103".to_owned())]);
}

/// Verifies a Soft unit must stop on entry into an enemy zone of control.
#[test]
fn a_soft_unit_must_stop_after_entering_an_enemy_zone_of_control() {
    let soft = "soviet.7guardsAirborneDivision.119regiment";
    let mut game = baltap_planning("warsawPact", &[(soft, "4009"), (GERMAN, "4012")]);
    game.execute(move_to(soft, "4011")).unwrap();
    assert_eq!(
        game.execute(move_to(soft, "4010")).unwrap_err().code,
        "mustStopInEnemyZoc"
    );

    // Hard units may keep moving, paying +1 to leave the EZOC hex.
    let mut game = baltap_planning("warsawPact", &[(SOVIET, "4009"), (GERMAN, "4012")]);
    game.execute(move_to(SOVIET, "4011")).unwrap();
    let outcome = game.execute(move_to(SOVIET, "4010")).unwrap();
    assert_eq!(outcome.snapshot.battle_plan.unwrap().movements[1].cost, 2);
}

/// Verifies movement previews and commands accept exactly the same destination hexes.
#[test]
fn every_movement_option_is_accepted_and_no_other_hex_is() {
    let mut game = baltap_planning("warsawPact", &[(SOVIET, "2412"), (GERMAN, "2415")]);
    let unit_id = UnitId(SOVIET.to_owned());
    // March is unavailable in the contested Airspace around Schwerin, so the
    // second pass checks it from the rear.
    for (mode, origin) in [
        (MovementMode::Tactical, "2412"),
        (MovementMode::March, "2604"),
    ] {
        game.units.get_mut(&unit_id).unwrap().location = UnitLocation::Hex {
            hex_id: HexId(origin.to_owned()),
        };
        let options = game.movement_options(&unit_id, mode).unwrap();
        assert!(!options.is_empty());
        // Every hex within reach of a doubled allowance, plus a margin.
        let hexes: Vec<HexId> = game
            .map()
            .hexes
            .iter()
            .filter(|hex| hex.row.abs_diff(24) <= 12 && hex.col.abs_diff(12) <= 12)
            .map(|hex| hex.id.clone())
            .collect();
        for hex_id in hexes {
            let offered = options.iter().find(|option| option.hex_id == hex_id);
            let result = game.execute(GameCommand::MoveUnit {
                unit_id: unit_id.clone(),
                destination: hex_id.clone(),
                mode,
            });
            match (offered, result) {
                (Some(option), Ok(outcome)) => {
                    let movement = outcome.snapshot.battle_plan.unwrap().movements[0].clone();
                    assert_eq!(
                        (movement.cost, movement.path),
                        (option.cost, option.path.clone())
                    );
                    game.execute(GameCommand::UndoUnitMovement {
                        unit_id: unit_id.clone(),
                    })
                    .unwrap();
                }
                (None, Err(_)) => {}
                (offered, result) => panic!(
                    "{mode:?} to {}: offered={} accepted={}",
                    hex_id.0,
                    offered.is_some(),
                    result.is_ok()
                ),
            }
        }
    }
}

/// Copies movement orders from the current battle plan for test assertions.
fn plan_movements(game: &GameEngine) -> Vec<crate::PlannedMovement> {
    game.snapshot().battle_plan.unwrap().movements
}

/// Returns the snapshot's controlling side identifier for a city hex.
fn controller(game: &GameEngine, hex: &str) -> String {
    game.snapshot()
        .cities
        .into_iter()
        .find(|city| city.hex_id.0 == hex)
        .unwrap()
        .controller
        .0
}

/// Verifies each city starts controlled by its original alliance with Free City status.
#[test]
fn every_city_starts_as_a_free_city_of_its_original_alliance() {
    let snapshot = new_game().snapshot();
    assert_eq!(snapshot.cities.len(), 94);
    assert!(snapshot.cities.iter().all(|city| city.free));
    let owner = |hex: &str| {
        snapshot
            .cities
            .iter()
            .find(|city| city.hex_id.0 == hex)
            .unwrap()
            .owner
            .0
            .clone()
    };
    assert_eq!(owner("2214"), "nato"); // Lübeck
    assert_eq!(owner("3007"), "nato"); // West Berlin
    assert_eq!(owner("3006"), "warsawPact"); // Ost Berlin
    assert_eq!(owner("2111"), "warsawPact"); // Rostock
}

/// Verifies ground movement cannot enter an enemy Free City.
#[test]
fn ground_movement_may_not_enter_an_enemy_free_city() {
    let mut game = baltap_planning("warsawPact", &[(SOVIET, "2314")]);
    let modes = game.movement_modes(&UnitId(SOVIET.to_owned())).unwrap();
    for mode in &modes {
        assert!(mode.options.iter().all(|option| option.hex_id.0 != "2214"));
    }
    assert_eq!(
        game.execute(move_to(SOVIET, "2214")).unwrap_err().code,
        "enemyFreeCity"
    );
}

/// Verifies an unoccupied enemy Free City can still be selected as an attack objective.
#[test]
fn an_unoccupied_enemy_free_city_is_a_legal_attack_objective() {
    let mut game = baltap_planning("warsawPact", &[(SOVIET, "2314")]);
    let options = game.attack_target_options().unwrap();
    assert!(options.contains(&HexId("2214".to_owned())));
    assert!(!options.contains(&HexId("2215".to_owned())));
    assert!(!options.contains(&HexId("2111".to_owned()))); // own Free City
    game.execute(GameCommand::SetAttackTarget {
        hex_id: HexId("2214".to_owned()),
        selected: true,
    })
    .unwrap();
    assert_eq!(
        game.execute(GameCommand::SetAttackTarget {
            hex_id: HexId("2215".to_owned()),
            selected: true,
        })
        .unwrap_err()
        .code,
        "invalidAttackTarget"
    );
}

/// Verifies Tactical movement liberates a conquered city and undo restores its previous controller.
#[test]
fn tactical_movement_liberates_a_conquered_city_and_undo_restores_it() {
    let mut game = baltap_planning("nato", &[(DANE, "2215")]);
    game.city_control
        .insert(HexId("2214".to_owned()), SideId("warsawPact".to_owned()));
    let modes = game.movement_modes(&UnitId(DANE.to_owned())).unwrap();
    let march = modes
        .iter()
        .find(|mode| mode.mode == MovementMode::March)
        .unwrap();
    assert!(march.options.iter().all(|option| option.hex_id.0 != "2214"));

    let outcome = game.execute(move_to(DANE, "2214")).unwrap();
    assert!(outcome.events.contains(&GameEvent::CityControlChanged {
        hex_id: HexId("2214".to_owned()),
        controller: SideId("nato".to_owned()),
        free: true,
    }));
    assert_eq!(controller(&game, "2214"), "nato");
    assert_eq!(plan_movements(&game)[0].city_control_changes.len(), 1);

    game.execute(GameCommand::UndoUnitMovement {
        unit_id: UnitId(DANE.to_owned()),
    })
    .unwrap();
    assert_eq!(controller(&game, "2214"), "warsawPact");
}

/// Verifies movement-mode previews include structured reasons for unavailable transport systems.
#[test]
fn movement_modes_explain_unavailable_systems() {
    let game = baltap_planning("nato", &[("denmark.landzealand.hq", "1411")]);
    let modes = game
        .movement_modes(&UnitId("denmark.landzealand.hq".to_owned()))
        .unwrap();
    let reason = |mode: MovementMode| {
        modes
            .iter()
            .find(|entry| entry.mode == mode)
            .unwrap()
            .unavailable
            .as_ref()
            .map(|error| error.code.clone())
    };
    assert_eq!(reason(MovementMode::Tactical), None);
    assert_eq!(
        reason(MovementMode::March).as_deref(),
        Some("marchUnavailable")
    );
    assert_eq!(
        reason(MovementMode::Rail).as_deref(),
        Some("unitNotEntrained")
    );
    assert_eq!(
        reason(MovementMode::AirTransport).as_deref(),
        Some("unitNotAirTransportable")
    );
}

/// Verifies rail capacity counts completed Entrained steps rather than units still Entraining.
#[test]
fn only_entrained_units_count_against_rail_capacity() {
    let mut game = GameEngine::new(
        GameId("rail-capacity".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
    )
    .unwrap();
    let stack: Vec<UnitId> = game
        .snapshot()
        .units
        .into_iter()
        .filter(|unit| matches!(&unit.location, UnitLocation::Hex { hex_id } if hex_id.0 == "2412"))
        .map(|unit| unit.id().clone())
        .collect();
    let steps = |game: &GameEngine, status: TrainStatus| -> u16 {
        game.units
            .values()
            .filter(|unit| unit.train_status == Some(status))
            .map(UnitState::step_count)
            .sum()
    };
    // Move the stack to the rear, in friendly Airspace, where it may entrain.
    for unit_id in &stack {
        game.units.get_mut(unit_id).unwrap().location = UnitLocation::Hex {
            hex_id: HexId("2604".to_owned()),
        };
    }
    // Entraining markers do not count, so the whole stack may start loading.
    for unit_id in &stack {
        game.execute(GameCommand::EntrainUnit {
            unit_id: unit_id.clone(),
        })
        .unwrap();
    }
    assert!(steps(&game, TrainStatus::Entraining) > 8);

    for _ in 0..8 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    let entrained = steps(&game, TrainStatus::Entrained);
    assert!(
        entrained > 0 && entrained <= 8,
        "{entrained} steps entrained"
    );
    assert!(steps(&game, TrainStatus::Entraining) > 0);
}

/// Verifies Marsh and Clear terrain have equal ground movement costs.
#[test]
fn marsh_costs_the_same_as_clear_terrain() {
    let game = baltap_planning("nato", &[(GERMAN, "2922")]);
    let marsh = game
        .movement_options(&UnitId(GERMAN.to_owned()), MovementMode::Tactical)
        .unwrap()
        .into_iter()
        .find(|option| option.hex_id.0 == "2921")
        .unwrap();
    assert_eq!(marsh.cost, 1);
}

/// Verifies BALTAP gives Warsaw Pact three Airlift Commands and rejects excess use.
#[test]
fn baltap_warsaw_pact_has_three_airlift_commands() {
    let mut game = GameEngine::new(
        GameId("airlift-capacity".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
    )
    .unwrap();
    let airborne: Vec<UnitId> = game
        .units
        .values()
        .filter(|unit| {
            unit.definition.side_id.0 == "warsawPact"
                && unit.location == UnitLocation::StrategicReserve
                && unit.is_air_transportable()
                && unit.step_count() == 1
        })
        .map(|unit| unit.id().clone())
        .collect();
    assert!(airborne.len() >= 4);
    // Rostock, Szczecin, and Ost Berlin: one step each fills the three commands.
    for (unit_id, destination) in airborne.iter().zip(["2111", "2504", "3006"]) {
        game.execute(GameCommand::MoveUnit {
            unit_id: unit_id.clone(),
            destination: HexId(destination.to_owned()),
            mode: MovementMode::AirTransport,
        })
        .unwrap();
    }
    assert_eq!(
        game.execute(GameCommand::MoveUnit {
            unit_id: airborne[3].clone(),
            destination: HexId("3108".to_owned()),
            mode: MovementMode::AirTransport,
        })
        .unwrap_err()
        .code,
        "airliftCapacityExceeded"
    );
}

// ------------------------------------------------------------------ strikes

/// Creates a BALTAP test game and advances it to the requested side and phase.
fn baltap_at(side: &str, phase: &str) -> GameEngine {
    let mut game = GameEngine::new(
        GameId("strike-game".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
    )
    .unwrap();
    advance_to(&mut game, side, phase);
    game
}

/// Ends phases until the test game reaches the requested acting side and phase.
fn advance_to(game: &mut GameEngine, side: &str, phase: &str) {
    loop {
        let step = game.snapshot().turn.current_step.unwrap();
        if step.phase_id.0 == phase
            && matches!(&step.actor, PhaseActor::Side { side_id } if side_id.0 == side)
        {
            return;
        }
        game.execute(GameCommand::EndPhase).unwrap();
    }
}

/// Builds an Air Strike command with an ordered target list and chosen Air Point kind.
fn air_strike(hex: &str, units: &[&str], air_point: AirPointKind) -> GameCommand {
    GameCommand::PlanAirStrike {
        hex_id: HexId(hex.to_owned()),
        unit_ids: units.iter().map(|id| UnitId((*id).to_owned())).collect(),
        air_point,
    }
}

const PZG_16: &str = "westGermany.6panzergrenadierDivision.16panzergrenadierBrigade";
const PZG_17: &str = "westGermany.6panzergrenadierDivision.17panzergrenadierBrigade";

/// Dice whose next roll is `roll`.
fn dice_rolling(roll: u8) -> Dice {
    (0..)
        .map(|seed: u32| Dice::from_seed_text(&seed.to_string()))
        .find(|dice| dice.clone().d6() == roll)
        .unwrap()
}

/// Verifies modified die rolls map to the one-point Strike Table results.
#[test]
fn strike_table_matches_the_one_point_column() {
    use crate::strikes::strike_table;
    assert_eq!(strike_table(-1), StrikeResult::NoEffect);
    assert_eq!(strike_table(1), StrikeResult::NoEffect);
    assert_eq!(strike_table(2), StrikeResult::Disrupted);
    assert_eq!(strike_table(4), StrikeResult::Disrupted);
    assert_eq!(strike_table(5), StrikeResult::StepLoss);
    assert_eq!(strike_table(8), StrikeResult::StepLoss);
}

/// Verifies recurring Air Points reset while unspent bonus Tactical points carry over.
#[test]
fn baltap_air_points_reset_each_turn_and_the_bonus_is_kept() {
    let game = baltap_at("warsawPact", "offensiveStrike");
    for points in game.snapshot().air_points {
        assert_eq!(
            (points.tactical, points.operational, points.bonus_tactical),
            (1, 0, 1)
        );
    }
}

/// Verifies air mission targeting limits and cancellation refunds to the correct Air Point pool.
#[test]
fn air_strikes_respect_targeting_limits_and_refund_on_cancel() {
    let mut game = baltap_at("warsawPact", "offensiveStrike");
    // Two one-step brigades in one strike.
    game.execute(air_strike(
        "2415",
        &[PZG_16, PZG_17],
        AirPointKind::Tactical,
    ))
    .unwrap();
    // No unit may be struck twice in the segment.
    assert_eq!(
        game.execute(air_strike("2415", &[PZG_16], AirPointKind::Tactical))
            .unwrap_err()
            .code,
        "unitAlreadyTargeted"
    );
    // An HQ needs Operational Air Points.
    assert_eq!(
        game.execute(air_strike(
            "2117",
            &["westGermany.landjut.hq"],
            AirPointKind::Tactical
        ))
        .unwrap_err()
        .code,
        "operationalPointRequired"
    );
    // Tactical Air Points never reach enemy Airspace (Sjælland, far from WP sources).
    assert_eq!(
        game.execute(air_strike(
            "1411",
            &["denmark.landzealand.1mechanizedBrigade"],
            AirPointKind::Tactical
        ))
        .unwrap_err()
        .code,
        "enemyAirspace"
    );
    // The second Tactical point comes from the one-time bonus; then none are left.
    game.execute(air_strike(
        "2216",
        &["westGermany.6panzergrenadierDivision.18panzerBrigade"],
        AirPointKind::Tactical,
    ))
    .unwrap();
    let points = |game: &GameEngine| {
        let points = game.snapshot().air_points;
        let wp = points.iter().find(|p| p.side_id.0 == "warsawPact").unwrap();
        (wp.tactical, wp.bonus_tactical)
    };
    assert_eq!(points(&game), (0, 0));
    let plan = game.snapshot().strike_plan.unwrap();
    assert_eq!(plan.missions[1].source, AirPointSource::BonusTactical);

    game.execute(GameCommand::CancelAirMission {
        mission_id: plan.missions[1].id,
    })
    .unwrap();
    assert_eq!(points(&game), (0, 1));
}

/// Verifies deterministic strike rolls use the target modifier shown by the preview.
#[test]
fn air_strike_resolution_is_deterministic_and_uses_the_previewed_modifier() {
    let mut game = baltap_at("warsawPact", "offensiveStrike");
    let preview = game.air_strike_options().unwrap();
    let target = preview
        .targets
        .iter()
        .find(|target| target.hex_id.0 == "2415")
        .unwrap();
    let modifier = target.units.iter().map(|unit| unit.modifier).min().unwrap();
    // Surprise gives the WP +1 on turn 1.
    assert!(target.units.iter().all(|unit| unit.modifier >= 1));

    game.execute(air_strike(
        "2415",
        &[PZG_16, PZG_17],
        AirPointKind::Tactical,
    ))
    .unwrap();
    let expected_roll = game.dice.clone().d6();
    let outcome = game.execute(GameCommand::ResolveAirStrikes).unwrap();
    let resolution = outcome
        .events
        .iter()
        .find_map(|event| match event {
            GameEvent::AirStrikeResolved { resolution, .. } => Some(resolution.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(resolution.die_roll, expected_roll);
    assert_eq!(resolution.modifier, modifier);
    assert_eq!(
        resolution.result,
        crate::strikes::strike_table(expected_roll as i8 + modifier)
    );
    let plan = outcome.snapshot.strike_plan.unwrap();
    assert!(plan.resolved);
    assert_eq!(plan.missions[0].resolution, Some(resolution));
    // No further missions after resolution.
    assert_eq!(
        game.execute(air_strike(
            "2216",
            &["westGermany.6panzergrenadierDivision.18panzerBrigade"],
            AirPointKind::Tactical
        ))
        .unwrap_err()
        .code,
        "strikesResolved"
    );
}

/// Verifies eliminating a hex's last enemy unit by a strike places a Breakthrough Marker.
#[test]
fn a_step_loss_on_the_last_unit_in_a_hex_leaves_a_breakthrough_marker() {
    let mut game = baltap_at("warsawPact", "offensiveStrike");
    game.execute(air_strike(
        "2216",
        &["westGermany.6panzergrenadierDivision.18panzerBrigade"],
        AirPointKind::Tactical,
    ))
    .unwrap();
    game.dice = dice_rolling(6);
    let outcome = game.execute(GameCommand::ResolveAirStrikes).unwrap();
    let brigade = UnitId("westGermany.6panzergrenadierDivision.18panzerBrigade".to_owned());
    assert!(outcome.events.contains(&GameEvent::UnitEliminated {
        unit_id: brigade.clone(),
        hex_id: HexId("2216".to_owned()),
    }));
    assert_eq!(outcome.snapshot.eliminated_unit_ids, vec![brigade]);
    assert_eq!(
        outcome.snapshot.breakthrough_markers,
        vec![HexId("2216".to_owned())]
    );
    // Breakthrough Markers come off at the end of the WP Reserve Phase.
    advance_to(&mut game, "warsawPact", "reserve");
    game.execute(GameCommand::EndPhase).unwrap();
    assert!(game.snapshot().breakthrough_markers.is_empty());
}

/// Verifies Disrupted units are limited to Minimum movement until their markers are removed.
#[test]
fn disrupted_units_use_minimum_movement_until_their_recovery() {
    let mut game = baltap_at("warsawPact", "offensiveStrike");
    game.execute(air_strike(
        "2415",
        &[PZG_16, PZG_17],
        AirPointKind::Tactical,
    ))
    .unwrap();
    game.dice = dice_rolling(3); // 3 + modifiers stays in the Disrupted band.
    let outcome = game.execute(GameCommand::ResolveAirStrikes).unwrap();
    assert!(outcome.events.contains(&GameEvent::UnitDisruptionChanged {
        unit_id: UnitId(PZG_16.to_owned()),
        disruption: Some(Disruption::Disrupted),
    }));

    advance_to(&mut game, "nato", "battlePlanning");
    let modes = game.movement_modes(&UnitId(PZG_16.to_owned())).unwrap();
    let mode = |mode: MovementMode| modes.iter().find(|entry| entry.mode == mode).unwrap();
    assert_eq!(
        mode(MovementMode::March).unavailable.as_ref().unwrap().code,
        "unitDisrupted"
    );
    // Only single-hex Minimum movement remains.
    assert!(mode(MovementMode::Tactical)
        .options
        .iter()
        .all(|option| option.path.len() == 1));

    // Recovery at the end of Battle Planning removes the marker.
    game.execute(GameCommand::EndPhase).unwrap();
    let unit = game
        .snapshot()
        .units
        .into_iter()
        .find(|unit| unit.id().0 == PZG_16)
        .unwrap();
    assert_eq!(unit.disruption, None);
}

/// Verifies enemy Air Interdiction Zones add Tactical movement costs and block March movement.
#[test]
fn air_interdiction_zones_slow_tactical_and_bar_march_movement() {
    let mut game = baltap_planning("nato", &[(GERMAN, "3321")]);
    let before = game
        .movement_options(&UnitId(GERMAN.to_owned()), MovementMode::Tactical)
        .unwrap()
        .into_iter()
        .find(|option| option.hex_id.0 == "3320")
        .unwrap()
        .cost;
    game.air_interdiction_zones
        .push(crate::AirInterdictionZone {
            side_id: SideId("warsawPact".to_owned()),
            hex_id: HexId("3320".to_owned()),
        });
    let tactical = game
        .movement_options(&UnitId(GERMAN.to_owned()), MovementMode::Tactical)
        .unwrap();
    assert_eq!(
        tactical
            .iter()
            .find(|option| option.hex_id.0 == "3320")
            .unwrap()
            .cost,
        before + 1
    );
    let march = game
        .movement_options(&UnitId(GERMAN.to_owned()), MovementMode::March)
        .unwrap();
    assert!(march.iter().all(|option| option.hex_id.0 != "3320"));
}

/// Verifies interdiction starts on mission resolution and expires after the enemy Reserve Phase.
#[test]
fn interdiction_is_placed_on_resolution_and_removed_after_the_enemy_reserve_phase() {
    let mut game = baltap_at("warsawPact", "offensiveStrike");
    game.execute(GameCommand::PlanAirInterdiction {
        hex_id: HexId("2414".to_owned()),
        air_point: AirPointKind::Tactical,
    })
    .unwrap();
    // Ending the phase resolves pending missions.
    game.execute(GameCommand::EndPhase).unwrap();
    assert_eq!(game.snapshot().air_interdiction_zones.len(), 1);
    // The zone survives the WP Reserve Phase and is removed after NATO's.
    advance_to(&mut game, "nato", "battlePlanning");
    assert_eq!(game.snapshot().air_interdiction_zones.len(), 1);
    advance_to(&mut game, "nato", "reserve");
    game.execute(GameCommand::EndPhase).unwrap();
    assert!(game.snapshot().air_interdiction_zones.is_empty());
}

/// Verifies West Berlin's city definition does not project airspace control.
#[test]
fn west_berlin_does_not_contest_airspace() {
    let game = baltap_at("warsawPact", "offensiveStrike");
    let airspace = game.airspace_map(&SideId("warsawPact".to_owned()));
    // Next to West Berlin, with no NATO unit nearby, the Airspace is WP-friendly.
    assert_eq!(airspace.of("3006"), Airspace::Friendly);
}

// ------------------------------------------------------------------ combat

/// BALTAP WP Combat Phase with isolated units, after marking `objective`.
fn baltap_combat(placements: &[(&str, &str)], objective: &str) -> GameEngine {
    let mut game = baltap_planning("warsawPact", placements);
    game.execute(GameCommand::SetAttackTarget {
        hex_id: HexId(objective.to_owned()),
        selected: true,
    })
    .unwrap();
    advance_to(&mut game, "warsawPact", "combat");
    game
}

/// Builds an unsupported battle command for a hex and its committed attacking units.
fn battle(hex: &str, units: &[&str]) -> GameCommand {
    GameCommand::ResolveBattle {
        hex_id: HexId(hex.to_owned()),
        unit_ids: units.iter().map(|id| UnitId((*id).to_owned())).collect(),
        supporting_hq_id: None,
    }
}

/// Extracts the resolved battle report from a command outcome's events.
fn report(outcome: &crate::CommandOutcome) -> crate::BattleReport {
    outcome
        .events
        .iter()
        .find_map(|event| match event {
            GameEvent::BattleResolved { report, .. } => Some(report.clone()),
            _ => None,
        })
        .unwrap()
}

const EAST_GERMAN: &str = "eastGermany.2gta.8motorRifleDivision";

/// Verifies printed combat codes parse into losses, disruption, counterattack, and retreat fields.
#[test]
fn combat_results_parse_from_printed_codes() {
    use crate::combat::parse_result;
    let r = parse_result("A1*/-");
    assert_eq!(
        (
            r.attacker_steps,
            r.attacker_disrupted,
            r.defender_steps,
            r.retreat
        ),
        (1, true, 0, 0)
    );
    let r = parse_result("-/CAD1R1");
    assert!(r.counterattack && r.attacker_steps == 0);
    assert_eq!(
        (r.defender_steps, r.retreat, r.defender_disrupted),
        (1, 1, false)
    );
    let r = parse_result("A1/D2*R2");
    assert_eq!(
        (
            r.attacker_steps,
            r.defender_steps,
            r.defender_disrupted,
            r.retreat
        ),
        (1, 2, true, 2)
    );
}

/// Verifies mandatory objectives prevent ending combat and battle odds use the printed rules.
#[test]
fn a_marked_objective_must_be_attacked_and_odds_follow_the_rules() {
    let mut game = baltap_combat(&[(SOVIET, "4009"), (GERMAN, "4010")], "4010");
    let options = game.combat_options().unwrap();
    assert_eq!(options.mandatory_remaining, vec![HexId("4010".to_owned())]);
    assert_eq!(
        game.execute(GameCommand::EndPhase).unwrap_err().code,
        "mandatoryAttacksRemaining"
    );
    // 8 against a Hard brigade's 3 in Clear terrain is 2:1; Surprise makes it 3:1.
    let odds = game
        .battle_preview(
            &HexId("4010".to_owned()),
            &[UnitId(SOVIET.to_owned())],
            None,
        )
        .unwrap();
    assert_eq!((odds.total_attack, odds.total_defense), (8, 3));
    assert_eq!(crate::ODDS_COLUMNS[odds.basic_column], "2:1");
    assert_eq!(odds.final_odds, "3:1");
    assert_eq!(odds.possible_results[5], "-/D1R1");
}

/// Verifies eliminating defenders offers an attacker advance and places a Breakthrough Marker.
#[test]
fn a_destroyed_defender_lets_the_attacker_advance_and_leaves_a_breakthrough() {
    let mut game = baltap_combat(&[(SOVIET, "4009"), (GERMAN, "4010")], "4010");
    game.dice = dice_rolling(6); // 3:1, roll 6: -/D1R1
    let outcome = game.execute(battle("4010", &[SOVIET])).unwrap();
    assert_eq!(report(&outcome).result.unwrap().code, "-/D1R1");
    assert!(outcome
        .snapshot
        .eliminated_unit_ids
        .contains(&UnitId(GERMAN.to_owned())));
    assert_eq!(
        outcome.snapshot.pending_decision.unwrap().kind,
        "advanceAfterCombat"
    );
    // Another battle may not start before the advance decision.
    assert_eq!(
        game.execute(battle("4010", &[SOVIET])).unwrap_err().code,
        "advancePending"
    );
    let outcome = game
        .execute(GameCommand::AdvanceAfterCombat {
            unit_ids: vec![UnitId(SOVIET.to_owned())],
        })
        .unwrap();
    let soviet = outcome
        .snapshot
        .units
        .iter()
        .find(|unit| unit.id().0 == SOVIET)
        .unwrap();
    assert_eq!(
        soviet.location,
        UnitLocation::Hex {
            hex_id: HexId("4010".to_owned())
        }
    );
    assert!(outcome
        .snapshot
        .breakthrough_markers
        .contains(&HexId("4010".to_owned())));
    game.execute(GameCommand::EndPhase).unwrap();
}

/// Verifies battle resolution applies attacking step losses and defending retreat movement.
#[test]
fn attacker_losses_and_defender_retreats_are_applied() {
    let mut game = baltap_combat(&[(SOVIET, "4009"), (GERMAN, "4010")], "4010");
    game.dice = dice_rolling(2); // 3:1, roll 2: A1/R1
    let outcome = game.execute(battle("4010", &[SOVIET])).unwrap();
    assert_eq!(report(&outcome).result.unwrap().code, "A1/R1");
    let unit = |id: &str| {
        outcome
            .snapshot
            .units
            .iter()
            .find(|unit| unit.id().0 == id)
            .cloned()
            .unwrap()
    };
    assert_eq!(unit(SOVIET).strength_step_index, 1);
    let UnitLocation::Hex { hex_id } = unit(GERMAN).location else {
        panic!("retreated off map");
    };
    assert_ne!(hex_id.0, "4010");
    assert!(outcome
        .events
        .iter()
        .any(|event| matches!(event, GameEvent::UnitRetreated { .. })));
    // Declining the advance still leaves a Breakthrough Marker in the empty hex.
    let outcome = game
        .execute(GameCommand::AdvanceAfterCombat { unit_ids: vec![] })
        .unwrap();
    assert!(outcome
        .snapshot
        .breakthrough_markers
        .contains(&HexId("4010".to_owned())));
}

/// Verifies a Counterattack result rolls once for every eligible defending step.
#[test]
fn a_counterattack_result_rolls_for_each_eligible_defending_step() {
    let mut game = baltap_combat(&[(SOVIET, "4009"), (GERMAN, "4010")], "4010");
    game.dice = dice_rolling(5); // 3:1, roll 5: -/CAD1R1
    let outcome = game.execute(battle("4010", &[SOVIET])).unwrap();
    let report = report(&outcome);
    assert_eq!(report.result.unwrap().code, "-/CAD1R1");
    assert_eq!(report.counterattacks.len(), 1);
    let roll = &report.counterattacks[0];
    // West German Counterattacks Disrupt on 3 or more.
    assert_eq!(roll.disrupted, roll.die_roll >= 3);
    let soviet = outcome
        .snapshot
        .units
        .iter()
        .find(|unit| unit.id().0 == SOVIET)
        .unwrap();
    assert_eq!(soviet.disruption.is_some(), roll.disrupted);
}

/// Verifies advancing into an undefended enemy Free City transfers its control.
#[test]
fn an_undefended_free_city_is_conquered_by_advancing() {
    let mut game = baltap_combat(&[(SOVIET, "2314")], "2214");
    let odds = game
        .battle_preview(
            &HexId("2214".to_owned()),
            &[UnitId(SOVIET.to_owned())],
            None,
        )
        .unwrap();
    assert_eq!(odds.city_defense, 1);
    let outcome = game.execute(battle("2214", &[SOVIET])).unwrap();
    // Every result at these odds inflicts a step loss, which breaks the city's defense.
    assert!(
        outcome
            .snapshot
            .combat
            .unwrap()
            .pending_advance
            .unwrap()
            .conquers_free_city
    );
    let outcome = game
        .execute(GameCommand::AdvanceAfterCombat {
            unit_ids: vec![UnitId(SOVIET.to_owned())],
        })
        .unwrap();
    let city = outcome
        .snapshot
        .cities
        .iter()
        .find(|city| city.hex_id.0 == "2214")
        .unwrap();
    assert_eq!(
        (city.controller.0.as_str(), city.free),
        ("warsawPact", false)
    );
}

/// Verifies surrounded defenders receive the appropriate Concentric or Flank odds shift.
#[test]
fn surrounded_defenders_suffer_concentric_or_flank_shifts() {
    let placements = [(SOVIET, "4009"), (EAST_GERMAN, "4011"), (GERMAN, "4010")];
    let game = baltap_combat(&placements, "4010");
    let odds = game
        .battle_preview(
            &HexId("4010".to_owned()),
            &[UnitId(SOVIET.to_owned())],
            None,
        )
        .unwrap();
    assert!(odds.shifts.iter().any(|shift| shift.reason
        == crate::ColumnShiftReason::ConcentricAttack
        && shift.shift == 2));
    // Surprise lifts the two-column cap for the WP: +2 Concentric +1 Surprise.
    assert_eq!(odds.net_shift, 3);

    let mut placements = placements.to_vec();
    placements.push((PZG_17, "3910"));
    let game = baltap_combat(&placements, "4010");
    let odds = game
        .battle_preview(
            &HexId("4010".to_owned()),
            &[UnitId(SOVIET.to_owned())],
            None,
        )
        .unwrap();
    assert!(odds
        .shifts
        .iter()
        .any(|shift| shift.reason == crate::ColumnShiftReason::FlankAttack));
}

/// Verifies an attacking unit and objective hex cannot be committed twice in one Combat Phase.
#[test]
fn units_and_hexes_fight_only_once_per_combat_phase() {
    let mut game = baltap_combat(&[(SOVIET, "4009"), (GERMAN, "4010")], "4010");
    game.dice = dice_rolling(1); // 3:1, roll 1: A1/-
    game.execute(battle("4010", &[SOVIET])).unwrap();
    assert_eq!(
        game.execute(battle("4010", &[SOVIET])).unwrap_err().code,
        "invalidObjective"
    );
    assert!(game
        .combat_options()
        .unwrap()
        .mandatory_remaining
        .is_empty());
    game.execute(GameCommand::EndPhase).unwrap();
}

// ------------------------------------------------------------------ offensive support

const NEGF_HQ: &str = "soviet.northernEastGermanyFront.hq";
const BALTIC_CORPS_TANKS: &str = "soviet.balticCorps.138tankRegiment";

/// Builds a battle command that commits the specified Offensive Support HQ.
fn supported_battle(hex: &str, units: &[&str], hq: &str) -> GameCommand {
    GameCommand::ResolveBattle {
        hex_id: HexId(hex.to_owned()),
        unit_ids: units.iter().map(|id| UnitId((*id).to_owned())).collect(),
        supporting_hq_id: Some(UnitId(hq.to_owned())),
    }
}

/// BALTAP WP Combat Phase after marking every hex in `objectives`.
fn baltap_combat_marking(placements: &[(&str, &str)], objectives: &[&str]) -> GameEngine {
    let mut game = baltap_planning("warsawPact", placements);
    for objective in objectives {
        game.execute(GameCommand::SetAttackTarget {
            hex_id: HexId((*objective).to_owned()),
            selected: true,
        })
        .unwrap();
    }
    advance_to(&mut game, "warsawPact", "combat");
    game
}

/// Verifies a Front HQ supplies a single Offensive Support column shift once per phase.
#[test]
fn a_front_hq_gives_one_battle_a_column_of_offensive_support() {
    let placements = [
        (SOVIET, "4009"),
        (GERMAN, "4010"),
        (EAST_GERMAN, "4013"),
        (PZG_17, "4012"),
        (NEGF_HQ, "4007"),
    ];
    let mut game = baltap_combat_marking(&placements, &["4010", "4012"]);
    let options = game.combat_options().unwrap();
    let objective = options
        .objectives
        .iter()
        .find(|objective| objective.hex_id.0 == "4010")
        .unwrap();
    assert_eq!(objective.support_hq_ids, vec![UnitId(NEGF_HQ.to_owned())]);

    let hq = UnitId(NEGF_HQ.to_owned());
    let odds = game
        .battle_preview(
            &HexId("4010".to_owned()),
            &[UnitId(SOVIET.to_owned())],
            Some(&hq),
        )
        .unwrap();
    assert!(odds.shifts.iter().any(|shift| shift.reason
        == crate::ColumnShiftReason::OffensiveSupport
        && shift.shift == 1));
    // 2:1, +1 Surprise, +1 Offensive Support.
    assert_eq!(odds.final_odds, "4:1");

    let outcome = game
        .execute(supported_battle("4010", &[SOVIET], NEGF_HQ))
        .unwrap();
    assert_eq!(report(&outcome).supporting_hq_id, Some(hq.clone()));
    if outcome.snapshot.pending_decision.is_some() {
        game.execute(GameCommand::AdvanceAfterCombat { unit_ids: vec![] })
            .unwrap();
    }
    // The HQ has supported its one battle this phase.
    assert_eq!(
        game.execute(supported_battle("4012", &[EAST_GERMAN], NEGF_HQ))
            .unwrap_err()
            .code,
        "supportUnavailable"
    );
    game.execute(battle("4012", &[EAST_GERMAN])).unwrap();
}

/// Verifies Offensive Support requires an eligible HQ and a committed Subordinate within range.
#[test]
fn offensive_support_needs_a_subordinate_in_range_and_a_ready_hq() {
    let hq = UnitId(NEGF_HQ.to_owned());
    // The Baltic Corps is not subordinate to the NEGF HQ.
    let game = baltap_combat(
        &[
            (BALTIC_CORPS_TANKS, "4009"),
            (GERMAN, "4010"),
            (NEGF_HQ, "4007"),
        ],
        "4010",
    );
    let preview = game.battle_preview(
        &HexId("4010".to_owned()),
        &[UnitId(BALTIC_CORPS_TANKS.to_owned())],
        Some(&hq),
    );
    assert_eq!(preview.unwrap_err().code, "supportUnavailable");

    // Seven hexes away is beyond the Support Range of 6.
    let game = baltap_combat(
        &[(SOVIET, "4009"), (GERMAN, "4010"), (NEGF_HQ, "4002")],
        "4010",
    );
    assert!(game.combat_options().unwrap().objectives[0]
        .support_hq_ids
        .is_empty());

    // A Suppressed HQ cannot support.
    let mut game = baltap_combat(
        &[(SOVIET, "4009"), (GERMAN, "4010"), (NEGF_HQ, "4007")],
        "4010",
    );
    game.units.get_mut(&hq).unwrap().disruption = Some(Disruption::Suppressed);
    assert!(game.combat_options().unwrap().objectives[0]
        .support_hq_ids
        .is_empty());
}

/// Verifies BALTAP's Northern East Germany Front HQ is immobile.
#[test]
fn the_negf_hq_may_not_move_in_baltap() {
    let game = baltap_planning("warsawPact", &[(NEGF_HQ, "2613")]);
    let modes = game.movement_modes(&UnitId(NEGF_HQ.to_owned())).unwrap();
    assert!(modes.iter().all(|mode| mode
        .unavailable
        .as_ref()
        .is_some_and(|error| error.code == "unitImmobile")));
}

// ------------------------------------------------------------------ reserve

/// Builds a command to select or deselect a unit's Reserve/OMG marker.
fn set_reserve(unit: &str, selected: bool) -> GameCommand {
    GameCommand::SetReserve {
        unit_id: UnitId(unit.to_owned()),
        selected,
    }
}

/// Copies the Reserve/OMG unit identifiers selected in the current battle plan.
fn reserve_ids(game: &GameEngine) -> Vec<UnitId> {
    game.snapshot().battle_plan.unwrap().reserve_unit_ids
}

/// Verifies Reserve/OMG selection requires no more than half the applicable movement allowance spent.
#[test]
fn reserve_status_allows_at_most_half_the_movement_allowance() {
    // Movement Allowance 6: up to 3 Movement Points keeps the unit eligible.
    let mut game = baltap_planning("nato", &[(GERMAN, "3321")]);
    let options = game
        .movement_options(&UnitId(GERMAN.to_owned()), MovementMode::Tactical)
        .unwrap();
    let near = options
        .iter()
        .find(|option| option.cost <= 3)
        .unwrap()
        .clone();
    game.execute(move_to(GERMAN, &near.hex_id.0)).unwrap();
    game.execute(set_reserve(GERMAN, true)).unwrap();
    assert_eq!(reserve_ids(&game), vec![UnitId(GERMAN.to_owned())]);

    // Moving on past half the allowance removes the marker.
    let far = game
        .movement_options(&UnitId(GERMAN.to_owned()), MovementMode::Tactical)
        .unwrap()
        .into_iter()
        .find(|option| near.cost + option.cost > 3 && option.path.len() > 1)
        .unwrap();
    let outcome = game.execute(move_to(GERMAN, &far.hex_id.0)).unwrap();
    assert!(outcome.events.contains(&GameEvent::ReserveStatusChanged {
        side_id: SideId("nato".to_owned()),
        unit_id: UnitId(GERMAN.to_owned()),
        selected: false,
    }));
    assert!(reserve_ids(&game).is_empty());
    assert_eq!(
        game.execute(set_reserve(GERMAN, true)).unwrap_err().code,
        "movedTooFar"
    );
    let preview = game.reserve_options().unwrap();
    assert_eq!(preview[0].unavailable.as_ref().unwrap().code, "movedTooFar");

    // Undoing the long move makes it eligible again (but does not re-mark it).
    game.execute(GameCommand::UndoUnitMovement {
        unit_id: UnitId(GERMAN.to_owned()),
    })
    .unwrap();
    assert!(reserve_ids(&game).is_empty());
    assert!(game.reserve_options().unwrap()[0].unavailable.is_none());
    game.execute(set_reserve(GERMAN, true)).unwrap();
    game.execute(set_reserve(GERMAN, false)).unwrap();
    assert!(reserve_ids(&game).is_empty());
}

/// Verifies headquarters and units in an enemy zone of control cannot receive Reserve/OMG markers.
#[test]
fn reserve_status_excludes_headquarters_and_units_in_an_enemy_zoc() {
    let mut game = baltap_planning(
        "warsawPact",
        &[(SOVIET, "4009"), (GERMAN, "4011"), (NEGF_HQ, "4005")],
    );
    assert_eq!(
        game.execute(set_reserve(NEGF_HQ, true)).unwrap_err().code,
        "notManeuverUnit"
    );
    game.execute(set_reserve(SOVIET, true)).unwrap();
    // Ending movement next to the enemy (in its ZOC) removes the marker.
    let outcome = game.execute(move_to(SOVIET, "4010")).unwrap();
    assert!(outcome.events.iter().any(|event| matches!(
        event,
        GameEvent::ReserveStatusChanged {
            selected: false,
            ..
        }
    )));
    assert_eq!(
        game.execute(set_reserve(SOVIET, true)).unwrap_err().code,
        "enemyZoneOfControl"
    );
}

/// Verifies Reserve/OMG units are excluded from combat previews and attacking commands.
#[test]
fn units_in_reserve_do_not_attack() {
    // Hamburg (2416) is an empty NATO Free City: its ZOC covers only its own hex,
    // so the adjacent Soviet division may be marked OMG.
    let mut game = baltap_planning("warsawPact", &[(SOVIET, "2415")]);
    game.execute(GameCommand::SetAttackTarget {
        hex_id: HexId("2416".to_owned()),
        selected: true,
    })
    .unwrap();
    game.execute(set_reserve(SOVIET, true)).unwrap();
    advance_to(&mut game, "warsawPact", "combat");
    let options = game.combat_options().unwrap();
    assert!(options
        .objectives
        .iter()
        .all(|objective| objective.hex_id.0 != "2416"));
    assert_eq!(
        game.execute(battle("2416", &[SOVIET])).unwrap_err().code,
        "invalidObjective"
    );
    // With no eligible attacker the marked objective does not block the phase.
    game.execute(GameCommand::EndPhase).unwrap();
}

/// Verifies marked reserves move again at half allowance using Tactical movement only.
#[test]
fn reserve_units_move_again_at_half_allowance_by_tactical_movement_only() {
    let mut game = baltap_planning("nato", &[(GERMAN, "3321"), (PZG_17, "3320")]);
    game.execute(set_reserve(GERMAN, true)).unwrap();
    advance_to(&mut game, "nato", "reserve");
    let reserve = game.snapshot().reserve.unwrap();
    assert_eq!(reserve.unit_ids, vec![UnitId(GERMAN.to_owned())]);

    let modes = game.movement_modes(&UnitId(GERMAN.to_owned())).unwrap();
    let mode = |mode: MovementMode| modes.iter().find(|entry| entry.mode == mode).unwrap();
    for other in [
        MovementMode::March,
        MovementMode::Rail,
        MovementMode::AirTransport,
    ] {
        assert_eq!(
            mode(other).unavailable.as_ref().unwrap().code,
            "reserveTacticalOnly"
        );
    }
    // Half of 6 is 3; only single-hex Minimum moves may cost more.
    let tactical = &mode(MovementMode::Tactical).options;
    assert!(!tactical.is_empty());
    assert!(tactical
        .iter()
        .all(|option| option.cost <= 3 || option.path.len() == 1));

    // Unmarked units stay put.
    let unmarked = game.movement_modes(&UnitId(PZG_17.to_owned())).unwrap();
    assert!(unmarked.iter().all(|entry| entry
        .unavailable
        .as_ref()
        .is_some_and(|error| error.code == "notInReserve")));

    let destination = tactical[0].hex_id.clone();
    game.execute(move_to(GERMAN, &destination.0)).unwrap();
    assert_eq!(game.snapshot().reserve.unwrap().movements.len(), 1);
    game.execute(GameCommand::UndoUnitMovement {
        unit_id: UnitId(GERMAN.to_owned()),
    })
    .unwrap();
    assert!(game.snapshot().reserve.unwrap().movements.is_empty());
    game.execute(move_to(GERMAN, &destination.0)).unwrap();

    // 28.2.5: the markers come off when the phase ends.
    let outcome = game.execute(GameCommand::EndPhase).unwrap();
    assert!(outcome.events.contains(&GameEvent::ReserveMarkersRemoved {
        side_id: SideId("nato".to_owned()),
        unit_ids: vec![UnitId(GERMAN.to_owned())],
    }));
    assert!(outcome.snapshot.reserve.is_none());
}

/// Verifies Hard reserve units ignore enemy ZOC movement surcharges inside Breakthrough Zones.
#[test]
fn hard_reserve_units_ignore_ezoc_costs_in_a_breakthrough_zone() {
    let mut game = baltap_planning("warsawPact", &[(SOVIET, "4009"), (GERMAN, "4012")]);
    game.execute(set_reserve(SOVIET, true)).unwrap();
    advance_to(&mut game, "warsawPact", "reserve");
    let cost_to = |game: &GameEngine, hex: &str| {
        game.movement_options(&UnitId(SOVIET.to_owned()), MovementMode::Tactical)
            .unwrap()
            .into_iter()
            .find(|option| option.hex_id.0 == hex)
            .map(|option| option.cost)
    };
    // 4011 is in the German unit's ZOC: entering it costs +1.
    let normal = cost_to(&game, "4011").unwrap();
    game.breakthrough_markers.push(HexId("4011".to_owned()));
    assert_eq!(cost_to(&game, "4011").unwrap() + 1, normal);
}

/// Verifies post-battle cleanup removes only the acting side's Suppressed markers.
#[test]
fn post_battle_removes_only_the_acting_sides_suppressed_markers() {
    let mut game = baltap_at("warsawPact", "reserve");
    let hq_of = |side: &str| {
        game.units
            .values()
            .find(|unit| unit.is_headquarters() && unit.definition.side_id.0 == side)
            .unwrap()
            .id()
            .clone()
    };
    let (wp_hq, nato_hq) = (hq_of("warsawPact"), hq_of("nato"));
    for hq in [&wp_hq, &nato_hq] {
        game.units.get_mut(hq).unwrap().disruption = Some(Disruption::Suppressed);
    }
    let outcome = game.execute(GameCommand::EndPhase).unwrap();
    assert!(outcome.events.contains(&GameEvent::UnitDisruptionChanged {
        unit_id: wp_hq.clone(),
        disruption: None,
    }));
    let disruption = |id: &UnitId| {
        outcome
            .snapshot
            .units
            .iter()
            .find(|unit| unit.id() == id)
            .unwrap()
            .disruption
    };
    assert_eq!(disruption(&wp_hq), None);
    assert_eq!(disruption(&nato_hq), Some(Disruption::Suppressed));
}

/// Verifies Reserve/OMG selection commands deserialize from camelCase IPC messages.
#[test]
fn set_reserve_deserializes_from_a_camel_case_ipc_message() {
    let command: GameCommand = serde_json::from_value(serde_json::json!({
        "type": "setReserve",
        "unitId": GERMAN,
        "selected": true,
    }))
    .unwrap();
    assert_eq!(command, set_reserve(GERMAN, true));
}

// ------------------------------------------------------------------ supply

const LJ_HQ: &str = "westGermany.landjut.hq";
const SOVIET_3: &str = "soviet.2gta.94guardsMotorRifleDivision";

fn check_supply(game: &mut GameEngine, side: &str) -> Vec<GameEvent> {
    let mut events = Vec::new();
    game.update_supply(&SideId(side.to_owned()), &mut events);
    events
}

fn supplied(game: &GameEngine, unit: &str) -> bool {
    let supply = &game.units[&UnitId(unit.to_owned())].supply;
    [supply.headquarters, supply.movement, supply.combat]
        .iter()
        .flatten()
        .all(|status| *status == SupplyStatus::Supplied)
}

#[test]
fn units_within_ten_hexes_of_a_friendly_free_city_are_supplied() {
    // 2617 is two hexes from Lüneburg; 2402 is more than ten from any NATO city.
    let mut game = baltap_planning("nato", &[(GERMAN, "2617"), (PZG_17, "2402")]);
    let events = check_supply(&mut game, "nato");
    assert!(supplied(&game, GERMAN));
    assert!(!supplied(&game, PZG_17));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::UnitSupplyChanged { unit_id, .. } if unit_id.0 == PZG_17
    )));
    // An unchanged unit produces no event.
    assert!(events.iter().all(|event| !matches!(
        event,
        GameEvent::UnitSupplyChanged { unit_id, .. } if unit_id.0 == GERMAN
    )));
}

#[test]
fn an_encircled_unit_is_out_of_supply() {
    // Three Soviet divisions two hexes away cover all six neighbours with their ZOCs.
    let mut game = baltap_planning(
        "nato",
        &[
            (GERMAN, "2617"),
            (SOVIET, "2516"),
            (SOVIET_2, "2519"),
            (SOVIET_3, "2817"),
        ],
    );
    check_supply(&mut game, "nato");
    assert!(!supplied(&game, GERMAN));
    // Opening one side of the ring restores the Line of Supply.
    game.units.remove(&UnitId(SOVIET_3.to_owned()));
    check_supply(&mut game, "nato");
    assert!(supplied(&game, GERMAN));
}

#[test]
fn a_supplied_hq_supplies_units_within_its_support_range() {
    // LANDJUT (Support Range 3) at 2305 reaches Lübeck and the brigade at 2402.
    let mut game = baltap_planning("nato", &[(PZG_17, "2402"), (LJ_HQ, "2305")]);
    check_supply(&mut game, "nato");
    assert!(supplied(&game, LJ_HQ));
    assert!(supplied(&game, PZG_17));

    // An HQ under a train marker supplies no one.
    game.units
        .get_mut(&UnitId(LJ_HQ.to_owned()))
        .unwrap()
        .train_status = Some(TrainStatus::Entraining);
    check_supply(&mut game, "nato");
    assert!(!supplied(&game, PZG_17));

    // Nor does an eliminated one.
    game.units
        .get_mut(&UnitId(LJ_HQ.to_owned()))
        .unwrap()
        .train_status = None;
    check_supply(&mut game, "nato");
    assert!(supplied(&game, PZG_17));
    game.units.remove(&UnitId(LJ_HQ.to_owned()));
    check_supply(&mut game, "nato");
    assert!(!supplied(&game, PZG_17));
}

#[test]
fn an_unsupplied_hq_supplies_no_one_and_moves_at_half_allowance() {
    let mut game = baltap_planning("nato", &[(LJ_HQ, "2402"), (PZG_17, "2403")]);
    check_supply(&mut game, "nato");
    assert!(!supplied(&game, LJ_HQ));
    assert!(!supplied(&game, PZG_17));
    // Movement Allowance 4 halves to 2; only a single-hex Minimum move may cost more.
    let options = game
        .movement_options(&UnitId(LJ_HQ.to_owned()), MovementMode::Tactical)
        .unwrap();
    assert!(!options.is_empty());
    assert!(options
        .iter()
        .all(|option| option.cost <= 2 || option.path.len() == 1));
}

#[test]
fn west_berlin_supplies_only_units_in_or_next_to_it() {
    let mut game = baltap_planning(
        "nato",
        &[(GERMAN, "3007"), (PZG_17, "3008"), (DANE, "3009")],
    );
    // Every hex near West Berlin is within ten hexes of a West German city, so
    // hand all other NATO cities to the WP to leave West Berlin as the only source.
    for (hex_id, controller) in game.city_control.iter_mut() {
        if hex_id.0 != "3007" && controller.0 == "nato" {
            *controller = SideId("warsawPact".to_owned());
        }
    }
    check_supply(&mut game, "nato");
    assert!(supplied(&game, GERMAN));
    assert!(supplied(&game, PZG_17));
    assert!(!supplied(&game, DANE));
}

#[test]
fn pre_battle_checks_the_acting_sides_supply() {
    let mut game = baltap_planning("nato", &[(PZG_17, "2402")]);
    assert!(supplied(&game, PZG_17));
    advance_to(&mut game, "warsawPact", "battlePlanning");
    // The WP Pre-Battle Phase does not check NATO units.
    assert!(supplied(&game, PZG_17));
    advance_to(&mut game, "nato", "battlePlanning");
    assert!(!supplied(&game, PZG_17));
}

// ------------------------------------------------------------------ setup

#[test]
fn a_setup_can_start_in_the_reserve_phase_with_its_own_markers() {
    let setup: crate::GameSetup = serde_json::from_value(serde_json::json!({
        "start": { "gameTurn": 2, "sideId": "nato", "phaseId": "reserve" },
        "units": [
            { "id": GERMAN, "hex": "3321" },
            { "id": PZG_17, "hex": "3320", "step": 0, "disruption": "disrupted" }
        ],
        "reserveUnitIds": [GERMAN],
        "breakthroughMarkers": ["3319"]
    }))
    .unwrap();
    let game = GameEngine::with_setup(
        GameId("setup".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
        &setup,
    )
    .unwrap();
    let snapshot = game.snapshot();
    assert_eq!(snapshot.turn.game_turn, 2);
    assert_eq!(snapshot.units.len(), 2);
    assert_eq!(
        snapshot.reserve.unwrap().unit_ids,
        vec![UnitId(GERMAN.to_owned())]
    );
    assert_eq!(
        snapshot.breakthrough_markers,
        vec![HexId("3319".to_owned())]
    );
    let disrupted = &game.units[&UnitId(PZG_17.to_owned())];
    assert_eq!(disrupted.disruption, Some(Disruption::Disrupted));

    // A new game opens in Battle Planning, so its plan takes attack targets.
    let setup: crate::GameSetup =
        serde_json::from_value(serde_json::json!({ "attackTargets": ["2415"] })).unwrap();
    let game = GameEngine::with_setup(
        GameId("setup".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
        &setup,
    )
    .unwrap();
    assert_eq!(
        game.snapshot().battle_plan.unwrap().attack_targets,
        vec![HexId("2415".to_owned())]
    );
}

// ------------------------------------------------------------------ lift movement

const AIRBORNE: &str = "soviet.7guardsAirborneDivision.119regiment";
const AIRMOBILE: &str = "unitedStates.9infantryDivision.1brigade";
const MARINE: &str = "poland.balticCorps.7marineBrigade";

fn lift(unit: &str, hex: &str, mode: MovementMode) -> GameCommand {
    GameCommand::MoveUnit {
        unit_id: UnitId(unit.to_owned()),
        destination: HexId(hex.to_owned()),
        mode,
    }
}

fn mode_error(game: &GameEngine, unit: &str, mode: MovementMode) -> Option<String> {
    game.movement_modes(&UnitId(unit.to_owned()))
        .unwrap()
        .into_iter()
        .find(|entry| entry.mode == mode)
        .unwrap()
        .unavailable
        .map(|error| error.code)
}

#[test]
fn air_transport_flies_city_to_city_and_may_cross_enemy_zones_of_control() {
    // Rostock to Schwerin, past a West German brigade at 2311.
    let mut game = baltap_planning("warsawPact", &[(AIRBORNE, "2111"), (GERMAN, "2311")]);
    let options = game
        .movement_options(&UnitId(AIRBORNE.to_owned()), MovementMode::AirTransport)
        .unwrap();
    let side = SideId("warsawPact".to_owned());
    assert!(options
        .iter()
        .all(|option| game.city_control.get(&option.hex_id) == Some(&side)));
    let schwerin = options
        .iter()
        .find(|option| option.hex_id.0 == "2412")
        .expect("Schwerin is reachable");
    assert!(schwerin
        .path
        .iter()
        .any(|hex| game.hex_in_enemy_zoc(&side, hex)));

    // Not to open country, nor to an enemy city.
    assert_eq!(
        game.execute(lift(AIRBORNE, "2413", MovementMode::AirTransport))
            .unwrap_err()
            .code,
        "invalidLiftDestination"
    );
    assert_eq!(
        game.execute(lift(AIRBORNE, "2417", MovementMode::AirTransport))
            .unwrap_err()
            .code,
        "enemyFreeCity"
    );
    game.execute(lift(AIRBORNE, "2412", MovementMode::AirTransport))
        .unwrap();
    assert_eq!(game.snapshot().battle_plan.unwrap().airlift_steps_used, 1);

    // Take-off conditions are unchanged: never from an EZOC.
    let game = baltap_planning("warsawPact", &[(AIRBORNE, "2111"), (GERMAN, "2110")]);
    assert_eq!(
        mode_error(&game, AIRBORNE, MovementMode::AirTransport).as_deref(),
        Some("enemyZoneOfControl")
    );
}

#[test]
fn airborne_units_may_paradrop_onto_clear_terrain_beside_the_enemy() {
    let mut game = baltap_planning("warsawPact", &[(AIRBORNE, "2111"), (GERMAN, "2415")]);
    let options = game
        .movement_options(&UnitId(AIRBORNE.to_owned()), MovementMode::Paradrop)
        .unwrap();
    let reaches = |hex: &str| options.iter().any(|option| option.hex_id.0 == hex);
    assert!(reaches("2414"), "Clear hex in the brigade's ZOC");
    assert!(
        !reaches("2415"),
        "no landing on the enemy (Assaults are not implemented)"
    );
    assert!(!reaches("2412"), "Forest is not a drop zone");
    game.execute(lift(AIRBORNE, "2414", MovementMode::Paradrop))
        .unwrap();
    assert_eq!(game.snapshot().battle_plan.unwrap().airlift_steps_used, 1);

    // Airmobile units fly but do not jump. (The brigade arrives later; a
    // setup places it in Kiel on turn one.)
    let setup: crate::GameSetup = serde_json::from_value(serde_json::json!({
        "start": { "gameTurn": 1, "sideId": "nato", "phaseId": "battlePlanning" },
        "units": [{ "id": AIRMOBILE, "hex": "2116" }]
    }))
    .unwrap();
    let game = GameEngine::with_setup(
        GameId("airmobile".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
        &setup,
    )
    .unwrap();
    assert_eq!(
        mode_error(&game, AIRMOBILE, MovementMode::Paradrop).as_deref(),
        Some("unitNotAirTransportable")
    );
    assert_eq!(
        mode_error(&game, AIRMOBILE, MovementMode::AirTransport),
        None
    );
}

#[test]
fn sea_transport_sails_port_to_port_from_the_strategic_reserve() {
    let mut game = baltap_planning("warsawPact", &[(MARINE, "2111")]);
    game.units
        .get_mut(&UnitId(MARINE.to_owned()))
        .unwrap()
        .location = UnitLocation::StrategicReserve;
    let options = game
        .movement_options(&UnitId(MARINE.to_owned()), MovementMode::SeaTransport)
        .unwrap();
    assert!(options.iter().any(|option| option.hex_id.0 == "2111"));
    assert!(options.iter().all(|option| {
        let hex = game
            .map()
            .hexes
            .iter()
            .find(|hex| hex.id == option.hex_id)
            .unwrap();
        hex.port.is_some()
    }));
    assert_eq!(
        game.execute(lift(MARINE, "2412", MovementMode::SeaTransport))
            .unwrap_err()
            .code,
        "invalidLiftDestination"
    );
    game.execute(lift(MARINE, "2111", MovementMode::SeaTransport))
        .unwrap();
    let plan = game.snapshot().battle_plan.unwrap();
    assert_eq!(plan.sealift_steps_used, 1);
    assert_eq!(plan.airlift_steps_used, 0);
}

#[test]
fn reinforcements_arrive_at_their_hex_spill_over_or_fall_back_to_reserve() {
    let mut scenario = find_scenario("nato-1983-standard").unwrap();
    let division = scenario.reinforcements[0].clone(); // two steps
    scenario
        .reinforcements
        .retain(|arrival| arrival.unit.definition.side_id.0 == "nato");
    let copy = |id: &str, hex: &str, train: Option<TrainStatus>| {
        let mut arrival = division.clone();
        arrival.game_turn = 2;
        arrival.unit.definition.id = UnitId(id.to_owned());
        arrival.unit.location = UnitLocation::Hex {
            hex_id: HexId(hex.to_owned()),
        };
        arrival.unit.train_status = train;
        arrival
    };
    scenario.reinforcements.extend([
        copy("test.a", "2806", None),
        copy("test.b", "2806", None),
        copy("test.c", "2806", None),
        // The West German brigade holds 3216.
        copy("test.d", "3216", None),
        copy("test.rail", "2706", Some(TrainStatus::Entrained)),
    ]);
    let mut game = GameEngine::new(GameId("arrivals".to_owned()), scenario).unwrap();
    for _ in 0..8 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    let location = |id: &str| game.units[&UnitId(id.to_owned())].location.clone();
    let at = |hex: &str| UnitLocation::Hex {
        hex_id: HexId(hex.to_owned()),
    };
    assert_eq!(location("test.a"), at("2806"));
    assert_eq!(location("test.b"), at("2806"));
    // Four steps fill 2806; the third division takes a neighbouring hex.
    let UnitLocation::Hex { hex_id } = location("test.c") else {
        panic!("spilled division should be on the map");
    };
    assert_ne!(hex_id.0, "2806");
    assert_eq!(location("test.d"), UnitLocation::StrategicReserve);
    assert_eq!(location("test.rail"), at("2706"));
    assert_eq!(
        game.units[&UnitId("test.rail".to_owned())].train_status,
        Some(TrainStatus::Entrained)
    );
}

#[test]
fn new_movement_modes_serialize_in_camel_case() {
    assert_eq!(
        serde_json::to_value(MovementMode::SeaTransport).unwrap(),
        "seaTransport"
    );
    assert_eq!(
        serde_json::to_value(MovementMode::Paradrop).unwrap(),
        "paradrop"
    );
}
