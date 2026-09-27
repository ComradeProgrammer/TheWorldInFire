use crate::dice::Dice;
use crate::model::{
    find_scenario, list_scenarios, AirPointKind, AirPointSource, Airspace, Disruption, HexId,
    MovementMode, PhaseActor, PhaseExecution, SideId, StrikeResult, SupplyStatus, TrainStatus,
    UnitId, UnitLocation, UnitState,
};
use crate::{GameCommand, GameEvent, GameId, GameState, GameStatus};

fn new_game() -> GameState {
    GameState::new(
        GameId("test-game".to_owned()),
        find_scenario("nato-1983-standard").unwrap(),
    )
    .unwrap()
}

#[test]
fn new_game_starts_at_first_scenario_defined_step() {
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
    assert_eq!(snapshot.protocol_version, 13);
    assert_eq!(snapshot.turn.game_turn, 1);
    assert_eq!(step.phase_id.0, "jointStatus");
    assert_eq!(step.actor, PhaseActor::All);
    assert_eq!(snapshot.scenario.map_id, "nato-central-europe");
    assert!(snapshot.units.is_empty());
}

#[test]
fn snapshot_identifies_the_scenario_map_without_repeating_its_data() {
    let json = serde_json::to_value(new_game().snapshot()).unwrap();

    assert_eq!(json["protocolVersion"], 13);
    assert_eq!(json["scenario"]["mapId"], "nato-central-europe");
    assert!(json.get("map").is_none());
}

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

#[test]
fn scenario_validation_rejects_a_unit_outside_its_map() {
    let mut scenario = find_scenario("nato-1983-standard").unwrap();
    scenario.reinforcements[0].unit.location = UnitLocation::Hex {
        hex_id: crate::HexId("9999".to_owned()),
    };

    let error = GameState::new(GameId("invalid-map-unit".to_owned()), scenario)
        .err()
        .expect("unknown deployment hex should be rejected");

    assert_eq!(error.code, "invalidScenario");
    assert!(error.message.contains("unknown map hex"));
}

#[test]
fn scenario_validation_rejects_a_hexside_outside_its_map() {
    let mut scenario = find_scenario("nato-1983-standard").unwrap();
    scenario.map.hexsides[0].a = crate::HexId("9999".to_owned());

    let error = GameState::new(GameId("invalid-map-hexside".to_owned()), scenario)
        .err()
        .expect("unknown hexside endpoint should be rejected");

    assert_eq!(error.code, "invalidScenario");
    assert!(error.message.contains("unknown hex"));
}

#[test]
fn opening_units_arrive_through_the_first_joint_reinforcement_phase() {
    let mut game = new_game();

    let outcome = game.execute(GameCommand::EndPhase).unwrap();

    assert_eq!(outcome.snapshot.units.len(), 2);
    assert_eq!(
        outcome.snapshot.turn.current_step.unwrap().phase_id.0,
        "battlePlanning"
    );
    assert!(matches!(
        &outcome.events[..],
        [
            GameEvent::PhaseEnded { step: ended, .. },
            GameEvent::PhaseStarted { step: reinforcement, .. },
            GameEvent::AirPointsReset { .. },
            GameEvent::ReinforcementsArrived { .. },
            GameEvent::PhaseEnded {
                step: reinforcement_ended,
                ..
            },
            GameEvent::PhaseStarted { step: pre_battle, .. },
            GameEvent::PreBattleSupplyChecked { side_id, units, .. },
            GameEvent::PhaseEnded {
                step: pre_battle_ended,
                ..
            },
            GameEvent::PhaseStarted { step: next, .. }
        ] if ended.phase_id.0 == "jointStatus"
            && reinforcement.phase_id.0 == "jointReinforcement"
            && reinforcement_ended.phase_id.0 == "jointReinforcement"
            && pre_battle.phase_id.0 == "preBattle"
            && pre_battle_ended.phase_id.0 == "preBattle"
            && side_id.0 == "warsawPact"
            && units.len() == 1
            && next.phase_id.0 == "battlePlanning"
    ));

    let arrivals = outcome.events.iter().find_map(|event| match event {
        GameEvent::ReinforcementsArrived { game_turn, units } => Some((game_turn, units)),
        _ => None,
    });
    let (game_turn, units) = arrivals.expect("opening reinforcement event");

    assert_eq!(*game_turn, 1);
    assert_eq!(units.len(), 2);
    assert!(units.iter().any(|unit| {
        unit.id().0 == "soviet.6thGuardsMotorRifleDivision"
            && unit.current_step().unwrap().attack == 8
            && unit.definition.steps.len() == 2
    }));
    assert!(units.iter().any(|unit| {
        unit.id().0 == "westGermany.1stBrigade.1stPanzerDivision"
            && unit.current_step().unwrap().movement == 6
    }));
}

#[test]
fn automatic_post_battle_is_processed_without_user_input() {
    let mut game = new_game();
    for _ in 0..4 {
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

#[test]
fn joint_phases_precede_both_side_turns() {
    let scenario = find_scenario("nato-1983-standard").unwrap();

    assert_eq!(scenario.turn_sequence[0].phase_id.0, "jointStatus");
    assert_eq!(scenario.turn_sequence[0].actor, PhaseActor::All);
    assert_eq!(scenario.turn_sequence[1].phase_id.0, "jointReinforcement");
    assert_eq!(scenario.turn_sequence[1].actor, PhaseActor::All);
    assert_eq!(scenario.turn_sequence[2].phase_id.0, "preBattle");
}

#[test]
fn final_turn_completes_after_last_interactive_phase_and_cleanup() {
    let mut game = new_game();
    let interactive_steps_per_turn = 9;
    for _ in 0..(14 * interactive_steps_per_turn) {
        game.execute(GameCommand::EndPhase).unwrap();
    }

    assert_eq!(game.snapshot().status, GameStatus::Completed);
    assert!(game.snapshot().turn.current_step.is_none());
}

#[test]
fn reinforcements_are_added_only_on_their_scheduled_turn() {
    let mut scenario = find_scenario("nato-1983-standard").unwrap();
    scenario.reinforcements[0].game_turn = 2;
    scenario.reinforcements.truncate(1);
    let unit_id = scenario.reinforcements[0].unit.id().clone();
    let mut game = GameState::new(GameId("scheduled-game".to_owned()), scenario).unwrap();

    game.execute(GameCommand::EndPhase).unwrap();
    assert!(game.snapshot().units.is_empty());

    for _ in 0..8 {
        game.execute(GameCommand::EndPhase).unwrap();
    }

    let snapshot = game.execute(GameCommand::EndPhase).unwrap().snapshot;
    assert_eq!(snapshot.turn.game_turn, 2);
    assert_eq!(snapshot.units.len(), 1);
    assert_eq!(snapshot.units[0].id(), &unit_id);
}

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

#[test]
fn pre_battle_supply_event_serializes_checked_unit_state() {
    let mut game = new_game();
    let outcome = game.execute(GameCommand::EndPhase).unwrap();
    let event = outcome
        .events
        .iter()
        .find(|event| matches!(event, GameEvent::PreBattleSupplyChecked { .. }))
        .expect("pre-battle supply event");

    let json = serde_json::to_value(event).unwrap();

    assert_eq!(json["type"], "preBattleSupplyChecked");
    assert_eq!(json["sideId"], "warsawPact");
    assert_eq!(
        json["units"][0]["unitId"],
        "soviet.6thGuardsMotorRifleDivision"
    );
    assert_eq!(json["units"][0]["supply"]["movement"], "supplied");
}

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

#[test]
fn baltap_opening_setup_and_later_reinforcements_use_the_same_phase() {
    let scenario = find_scenario("nato-baltap-1983").unwrap();
    let mut game = GameState::new(GameId("baltap-game".to_owned()), scenario).unwrap();

    let opening = game.execute(GameCommand::EndPhase).unwrap();
    assert_eq!(opening.snapshot.units.len(), 27);
    assert_eq!(
        opening
            .snapshot
            .units
            .iter()
            .filter(|unit| matches!(unit.location, UnitLocation::Hex { .. }))
            .count(),
        17
    );
    assert_eq!(
        opening
            .snapshot
            .units
            .iter()
            .filter(|unit| matches!(unit.location, UnitLocation::StrategicReserve))
            .count(),
        10
    );

    for _ in 0..8 {
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

#[test]
fn baltap_ends_after_seven_turns() {
    let scenario = find_scenario("nato-baltap-1983").unwrap();
    let mut game = GameState::new(GameId("baltap-completion".to_owned()), scenario).unwrap();

    for _ in 0..(7 * 9) {
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

#[test]
fn baltap_pre_battle_records_applicable_supply_types() {
    let scenario = find_scenario("nato-baltap-1983").unwrap();
    let mut game = GameState::new(GameId("baltap-supply".to_owned()), scenario).unwrap();

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

#[test]
fn battle_plan_actions_can_be_submitted_in_any_order() {
    let mut game = new_game();
    game.execute(GameCommand::EndPhase).unwrap();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());

    game.execute(GameCommand::SetAttackTarget {
        hex_id: HexId("3216".to_owned()),
        selected: true,
    })
    .unwrap();
    game.execute(GameCommand::SetResupplyTarget {
        unit_id: Some(unit_id.clone()),
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
    assert_eq!(plan.resupply_target_unit_id, Some(unit_id));
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

#[test]
fn resupply_is_applied_to_the_selected_stack_when_planning_ends() {
    let mut game = new_game();
    game.execute(GameCommand::EndPhase).unwrap();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());
    let unit = game.units.get_mut(&unit_id).unwrap();
    unit.supply.movement = Some(SupplyStatus::OutOfSupply);
    unit.supply.combat = Some(SupplyStatus::OutOfSupply);

    game.execute(GameCommand::SetResupplyTarget {
        unit_id: Some(unit_id.clone()),
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

#[test]
fn entraining_takes_one_player_turn_before_rail_movement_is_available() {
    let mut game = new_game();
    game.execute(GameCommand::EndPhase).unwrap();
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

    for _ in 0..9 {
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

#[test]
fn a_detrain_order_can_be_undone_during_the_same_plan() {
    let mut game = new_game();
    game.execute(GameCommand::EndPhase).unwrap();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());
    game.execute(GameCommand::EntrainUnit {
        unit_id: unit_id.clone(),
    })
    .unwrap();
    for _ in 0..9 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    let train_status = |game: &GameState| {
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

#[test]
fn airborne_reserve_unit_can_use_scenario_limited_air_transport() {
    let scenario = find_scenario("nato-baltap-1983").unwrap();
    let mut game = GameState::new(GameId("airlift-game".to_owned()), scenario).unwrap();
    game.execute(GameCommand::EndPhase).unwrap();
    let unit_id = UnitId("soviet.7guardsAirborneDivision.119regiment".to_owned());

    let outcome = game
        .execute(GameCommand::MoveUnit {
            unit_id: unit_id.clone(),
            destination: HexId("2010".to_owned()),
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
        UnitLocation::Hex { ref hex_id } if hex_id.0 == "2010"
    ));
    assert_eq!(outcome.snapshot.battle_plan.unwrap().airlift_steps_used, 1);
}

#[test]
fn planning_commands_are_rejected_outside_battle_planning() {
    let mut game = new_game();
    let error = game
        .execute(GameCommand::SetAttackTarget {
            hex_id: HexId("3216".to_owned()),
            selected: true,
        })
        .unwrap_err();
    assert_eq!(error.code, "wrongPhase");
    assert_eq!(game.snapshot().revision, 0);
}

#[test]
fn resupply_selection_can_be_cancelled() {
    let mut game = new_game();
    game.execute(GameCommand::EndPhase).unwrap();
    let unit_id = UnitId("soviet.6thGuardsMotorRifleDivision".to_owned());
    game.execute(GameCommand::SetResupplyTarget {
        unit_id: Some(unit_id),
    })
    .unwrap();

    let outcome = game
        .execute(GameCommand::SetResupplyTarget { unit_id: None })
        .unwrap();

    assert_eq!(
        outcome
            .snapshot
            .battle_plan
            .unwrap()
            .resupply_target_unit_id,
        None
    );
    assert!(matches!(
        outcome.events.as_slice(),
        [GameEvent::ResupplyTargetSet { unit_id: None, .. }]
    ));
}

#[test]
fn a_units_last_movement_can_be_undone() {
    let mut game = new_game();
    game.execute(GameCommand::EndPhase).unwrap();
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

fn baltap_planning(side: &str, placements: &[(&str, &str)]) -> GameState {
    let mut game = GameState::new(
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
const GERMAN: &str = "westGermany.6panzergrenadierDivision.16panzergrenadierBrigade";

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

fn plan_movements(game: &GameState) -> Vec<crate::PlannedMovement> {
    game.snapshot().battle_plan.unwrap().movements
}

fn controller(game: &GameState, hex: &str) -> String {
    game.snapshot()
        .cities
        .into_iter()
        .find(|city| city.hex_id.0 == hex)
        .unwrap()
        .controller
        .0
}

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

#[test]
fn only_entrained_units_count_against_rail_capacity() {
    let mut game = GameState::new(
        GameId("rail-capacity".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
    )
    .unwrap();
    game.execute(GameCommand::EndPhase).unwrap();
    let stack: Vec<UnitId> = game
        .snapshot()
        .units
        .into_iter()
        .filter(|unit| matches!(&unit.location, UnitLocation::Hex { hex_id } if hex_id.0 == "2412"))
        .map(|unit| unit.id().clone())
        .collect();
    let steps = |game: &GameState, status: TrainStatus| -> u16 {
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

    for _ in 0..9 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    let entrained = steps(&game, TrainStatus::Entrained);
    assert!(
        entrained > 0 && entrained <= 8,
        "{entrained} steps entrained"
    );
    assert!(steps(&game, TrainStatus::Entraining) > 0);
}

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

#[test]
fn baltap_warsaw_pact_has_three_airlift_commands() {
    let mut game = GameState::new(
        GameId("airlift-capacity".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
    )
    .unwrap();
    game.execute(GameCommand::EndPhase).unwrap();
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
    for (unit_id, destination) in airborne.iter().zip(["2010", "2009", "2008"]) {
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
            destination: HexId("1910".to_owned()),
            mode: MovementMode::AirTransport,
        })
        .unwrap_err()
        .code,
        "airliftCapacityExceeded"
    );
}

// ------------------------------------------------------------------ strikes

fn baltap_at(side: &str, phase: &str) -> GameState {
    let mut game = GameState::new(
        GameId("strike-game".to_owned()),
        find_scenario("nato-baltap-1983").unwrap(),
    )
    .unwrap();
    advance_to(&mut game, side, phase);
    game
}

fn advance_to(game: &mut GameState, side: &str, phase: &str) {
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
    let points = |game: &GameState| {
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

#[test]
fn west_berlin_does_not_contest_airspace() {
    let game = baltap_at("warsawPact", "offensiveStrike");
    let airspace = game.airspace_map(&SideId("warsawPact".to_owned()));
    // Next to West Berlin, with no NATO unit nearby, the Airspace is WP-friendly.
    assert_eq!(airspace.of("3006"), Airspace::Friendly);
}

// ------------------------------------------------------------------ combat

/// BALTAP WP Combat Phase with isolated units, after marking `objective`.
fn baltap_combat(placements: &[(&str, &str)], objective: &str) -> GameState {
    let mut game = baltap_planning("warsawPact", placements);
    game.execute(GameCommand::SetAttackTarget {
        hex_id: HexId(objective.to_owned()),
        selected: true,
    })
    .unwrap();
    advance_to(&mut game, "warsawPact", "combat");
    game
}

fn battle(hex: &str, units: &[&str]) -> GameCommand {
    GameCommand::ResolveBattle {
        hex_id: HexId(hex.to_owned()),
        unit_ids: units.iter().map(|id| UnitId((*id).to_owned())).collect(),
        supporting_hq_id: None,
    }
}

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

fn supported_battle(hex: &str, units: &[&str], hq: &str) -> GameCommand {
    GameCommand::ResolveBattle {
        hex_id: HexId(hex.to_owned()),
        unit_ids: units.iter().map(|id| UnitId((*id).to_owned())).collect(),
        supporting_hq_id: Some(UnitId(hq.to_owned())),
    }
}

/// BALTAP WP Combat Phase after marking every hex in `objectives`.
fn baltap_combat_marking(placements: &[(&str, &str)], objectives: &[&str]) -> GameState {
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

#[test]
fn the_negf_hq_may_not_move_in_baltap() {
    let game = baltap_planning("warsawPact", &[(NEGF_HQ, "2613")]);
    let modes = game.movement_modes(&UnitId(NEGF_HQ.to_owned())).unwrap();
    assert!(modes.iter().all(|mode| mode
        .unavailable
        .as_ref()
        .is_some_and(|error| error.code == "unitImmobile")));
}
