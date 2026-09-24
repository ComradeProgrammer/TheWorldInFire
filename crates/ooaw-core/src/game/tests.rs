use crate::scenario::{find_scenario, PhaseActor, SideId};

use super::{GameCommand, GameEvent, GameId, GameState, GameStatus};

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
}

#[test]
fn automatic_post_battle_is_processed_without_user_input() {
    let mut game = new_game();
    for _ in 0..6 {
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
    let interactive_steps_per_turn = 12;
    for _ in 0..(14 * interactive_steps_per_turn) {
        game.execute(GameCommand::EndPhase).unwrap();
    }

    assert_eq!(game.snapshot().status, GameStatus::Completed);
    assert!(game.snapshot().turn.current_step.is_none());
}
