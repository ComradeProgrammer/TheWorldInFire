use crate::model::{find_scenario, list_scenarios, PhaseActor, SideId, UnitLocation};
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
    let snapshot = game.snapshot();
    let step = snapshot.turn.current_step.unwrap();

    assert_eq!(snapshot.revision, 0);
    assert_eq!(snapshot.turn.game_turn, 1);
    assert_eq!(step.phase_id.0, "jointStatus");
    assert_eq!(step.actor, PhaseActor::All);
    assert!(snapshot.units.is_empty());
}

#[test]
fn opening_units_arrive_through_the_first_joint_reinforcement_phase() {
    let mut game = new_game();

    let outcome = game.execute(GameCommand::EndPhase).unwrap();

    assert_eq!(outcome.snapshot.units.len(), 2);
    assert_eq!(
        outcome.snapshot.turn.current_step.unwrap().phase_id.0,
        "preBattle"
    );
    assert!(matches!(
        &outcome.events[..],
        [
            GameEvent::PhaseEnded { step: ended, .. },
            GameEvent::PhaseStarted { step: reinforcement, .. },
            GameEvent::ReinforcementsArrived { .. },
            GameEvent::PhaseEnded {
                step: reinforcement_ended,
                ..
            },
            GameEvent::PhaseStarted { step: next, .. }
        ] if ended.phase_id.0 == "jointStatus"
            && reinforcement.phase_id.0 == "jointReinforcement"
            && reinforcement_ended.phase_id.0 == "jointReinforcement"
            && next.phase_id.0 == "preBattle"
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
    for _ in 0..5 {
        game.execute(GameCommand::EndPhase).unwrap();
    }
    assert_eq!(
        game.snapshot().turn.current_step.unwrap().phase_id.0,
        "reserve"
    );

    let outcome = game.execute(GameCommand::EndPhase).unwrap();
    let snapshot = outcome.snapshot;
    let step = snapshot.turn.current_step.unwrap();

    assert_eq!(step.phase_id.0, "preBattle");
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
    let interactive_steps_per_turn = 11;
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

    for _ in 0..11 {
        game.execute(GameCommand::EndPhase).unwrap();
    }

    let snapshot = game.snapshot();
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
    assert!(json.get("game_turn").is_none());
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

    for _ in 0..10 {
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

    for _ in 0..(7 * 11) {
        game.execute(GameCommand::EndPhase).unwrap();
    }

    assert_eq!(game.snapshot().status, GameStatus::Completed);
    assert_eq!(game.snapshot().units.len(), 44);
}
