use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

/// Child-process harness for exercising the engine's JSON protocol.
struct EngineProcess {
    /// Engine subprocess stopped and reaped when the test harness is dropped.
    child: Child,
    /// Buffered pipe used to send newline-delimited JSON requests to the engine.
    stdin: BufWriter<ChildStdin>,
    /// Buffered pipe used to read newline-delimited JSON engine responses.
    stdout: BufReader<ChildStdout>,
}

impl EngineProcess {
    /// Spawns the engine binary with piped stdin and stdout for protocol tests.
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ooaw-engine"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start ooaw-engine process");
        let stdin = child.stdin.take().expect("capture engine stdin");
        let stdout = child.stdout.take().expect("capture engine stdout");
        Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
        }
    }

    /// Writes and flushes one JSON request, then reads and parses its JSON response.
    fn send(&mut self, request: Value) -> Value {
        serde_json::to_writer(&mut self.stdin, &request).expect("serialize engine request");
        self.stdin.write_all(b"\n").expect("terminate request");
        self.stdin.flush().expect("flush engine request");

        let mut line = String::new();
        let bytes = self
            .stdout
            .read_line(&mut line)
            .expect("read engine response");
        assert_ne!(bytes, 0, "engine exited before responding");
        serde_json::from_str(&line).expect("engine response is JSON")
    }
}

impl Drop for EngineProcess {
    /// Stops and reaps the engine child process when the test harness is dropped.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Extracts event type strings from a successful protocol response.
fn event_types(response: &Value) -> Vec<&str> {
    response["events"]
        .as_array()
        .expect("response has events")
        .iter()
        .map(|event| event["type"].as_str().expect("event has a type"))
        .collect()
}

/// Verifies game creation, command events, revision checks, and snapshots through the JSON process protocol.
#[test]
fn commands_drive_the_engine_through_its_json_process_boundary() {
    let mut engine = EngineProcess::start();

    let before_start = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": 0,
        "command": { "type": "endPhase" }
    }));
    assert_eq!(before_start["type"], "error");
    assert_eq!(before_start["code"], "gameNotStarted");

    // The automatic opening (Joint Status, Joint Reinforcement, Pre-Battle) is
    // resolved at creation: play opens on the WP Battle Planning Phase.
    let started = engine.send(json!({
        "type": "newGame",
        "scenarioId": "nato-1983-standard",
        "gameId": "black-box-e2e"
    }));
    assert_eq!(started["type"], "gameStarted");
    assert_eq!(started["snapshot"]["revision"], 0);
    assert_eq!(
        started["snapshot"]["turn"]["currentStep"]["phaseId"],
        "battlePlanning"
    );
    assert_eq!(
        started["snapshot"]["turn"]["currentStep"]["actor"]["sideId"],
        "warsawPact"
    );
    assert_eq!(started["snapshot"]["units"].as_array().unwrap().len(), 2);
    assert_eq!(started["map"]["id"], "nato-central-europe");

    // A rule violation is rejected without changing the revision.
    let rejected = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": 0,
        "command": {
            "type": "setAttackTarget",
            "hexId": "2806",
            "selected": true
        }
    }));
    assert_eq!(rejected["type"], "error");
    assert_eq!(rejected["code"], "invalidAttackTarget");

    let planned = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": 0,
        "command": {
            "type": "setResupplyTarget",
            "unitId": "soviet.6thGuardsMotorRifleDivision",
            "selected": true
        }
    }));
    assert_eq!(planned["type"], "commandAccepted");
    assert_eq!(planned["revision"], 1);
    assert_eq!(event_types(&planned), vec!["resupplyTargetSet"]);
    assert_eq!(
        planned["snapshot"]["battlePlan"]["resupplyTargetUnitIds"],
        json!(["soviet.6thGuardsMotorRifleDivision"])
    );

    let stale = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": 0,
        "command": { "type": "endPhase" }
    }));
    assert_eq!(stale["type"], "error");
    assert_eq!(stale["code"], "revisionMismatch");

    // Ending planning applies the resupply and opens the Offensive Strike Phase.
    let strike = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": 1,
        "command": { "type": "endPhase" }
    }));
    assert_eq!(strike["type"], "commandAccepted");
    assert_eq!(strike["revision"], 2);
    assert_eq!(
        strike["snapshot"]["turn"]["currentStep"]["phaseId"],
        "offensiveStrike"
    );
    assert_eq!(
        event_types(&strike),
        vec!["unitsResupplied", "phaseEnded", "phaseStarted"]
    );

    let snapshot = engine.send(json!({ "type": "getSnapshot" }));
    assert_eq!(snapshot["type"], "snapshot");
    assert_eq!(snapshot["snapshot"]["revision"], 2);
    assert_eq!(
        snapshot["snapshot"]["battlePlan"]["resupplyTargetUnitIds"],
        json!(["soviet.6thGuardsMotorRifleDivision"])
    );
}

/// Submits a command at `revision` and returns the accepted response, or panics with the rejection.
fn accepted(engine: &mut EngineProcess, revision: &mut u64, command: Value) -> Value {
    let response = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": *revision,
        "command": command
    }));
    assert_eq!(response["type"], "commandAccepted", "rejected: {response}");
    *revision = response["revision"].as_u64().expect("accepted revision");
    response
}

/// Phase id of the current step in a response's snapshot.
fn phase(response: &Value) -> &str {
    response["snapshot"]["turn"]["currentStep"]["phaseId"]
        .as_str()
        .expect("current phase")
}

/// Plays a BALTAP Warsaw Pact turn through Reserve/OMG marking, the Reserve
/// Phase, Post-Battle, and NATO's supply check using only the JSON protocol.
#[test]
fn a_reserve_unit_moves_in_the_reserve_phase_and_supply_is_checked() {
    const OMG: &str = "eastGermany.2gta.8motorRifleDivision";
    const UNMARKED: &str = "soviet.2gta.21motorRifleDivision";
    let mut engine = EngineProcess::start();
    let started = engine.send(json!({
        "type": "newGame",
        "scenarioId": "nato-baltap-1983",
        "gameId": "black-box-reserve"
    }));
    assert_eq!(started["type"], "gameStarted");
    let mut revision = 0;

    // The game opens on WP Battle Planning; the WP marks one division OMG.
    assert_eq!(phase(&started), "battlePlanning");
    let marked = accepted(
        &mut engine,
        &mut revision,
        json!({ "type": "setReserve", "unitId": OMG, "selected": true }),
    );
    assert_eq!(event_types(&marked), vec!["reserveStatusChanged"]);
    assert_eq!(
        marked["snapshot"]["battlePlan"]["reserveUnitIds"],
        json!([OMG])
    );

    // Strike and Combat pass; the Reserve Phase opens with the marked unit.
    accepted(&mut engine, &mut revision, json!({ "type": "endPhase" }));
    accepted(&mut engine, &mut revision, json!({ "type": "endPhase" }));
    let reserve = accepted(&mut engine, &mut revision, json!({ "type": "endPhase" }));
    assert_eq!(phase(&reserve), "reserve");
    assert_eq!(reserve["snapshot"]["reserve"]["unitIds"], json!([OMG]));

    // Unmarked units stay put; marked ones move by Tactical movement only.
    let unmarked = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": revision,
        "command": { "type": "moveUnit", "unitId": UNMARKED, "destination": "2411", "mode": "tactical" }
    }));
    assert_eq!(unmarked["code"], "notInReserve");
    let march = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": revision,
        "command": { "type": "moveUnit", "unitId": OMG, "destination": "2111", "mode": "march" }
    }));
    assert_eq!(march["code"], "reserveTacticalOnly");
    let moved = accepted(
        &mut engine,
        &mut revision,
        json!({ "type": "moveUnit", "unitId": OMG, "destination": "2111", "mode": "tactical" }),
    );
    assert_eq!(event_types(&moved)[0], "unitMoved");
    assert_eq!(moved["snapshot"]["reserve"]["movements"][0]["to"], "2111");

    // Ending the Reserve Phase removes the marker, runs Post-Battle, and
    // checks NATO's supply in its Pre-Battle step.
    let ended = accepted(&mut engine, &mut revision, json!({ "type": "endPhase" }));
    let events = event_types(&ended);
    assert!(events.contains(&"reserveMarkersRemoved"));
    assert!(events.contains(&"preBattleSupplyChecked"));
    assert_eq!(ended["snapshot"]["reserve"], Value::Null);
    assert_eq!(ended["snapshot"]["battlePlan"]["sideId"], "nato");
    let nato_check = ended["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["type"] == "preBattleSupplyChecked")
        .unwrap();
    assert_eq!(nato_check["sideId"], "nato");
    // Nobody is cut off in the opening position.
    assert!(!events.contains(&"unitSupplyChanged"));
    assert!(nato_check["units"]
        .as_array()
        .unwrap()
        .iter()
        .all(|check| !check["supply"].to_string().contains("outOfSupply")));
}

/// Starts a game from `setup` (see `GameSetup`) and returns the `gameStarted` response.
fn start_game(engine: &mut EngineProcess, scenario_id: &str, game_id: &str, setup: Value) -> Value {
    let started = engine.send(json!({
        "type": "newGame",
        "scenarioId": scenario_id,
        "gameId": game_id,
        "setup": setup
    }));
    assert_eq!(started["type"], "gameStarted", "setup rejected: {started}");
    started
}

/// Finds a unit in a response's snapshot.
fn unit<'a>(response: &'a Value, id: &str) -> &'a Value {
    response["snapshot"]["units"]
        .as_array()
        .expect("snapshot units")
        .iter()
        .find(|unit| unit["id"] == id)
        .unwrap_or_else(|| panic!("unit {id} not in snapshot"))
}

/// A setup lays out an encirclement that ordinary play would take turns to
/// reach; NATO's next Pre-Battle supply check cuts the surrounded brigade off.
#[test]
fn a_setup_can_start_from_any_situation() {
    const BRIGADE: &str = "westGermany.6panzergrenadierDivision.16panzergrenadierBrigade";
    let mut engine = EngineProcess::start();
    let started = start_game(
        &mut engine,
        "nato-baltap-1983",
        "black-box-encircled",
        json!({
            "start": { "gameTurn": 1, "sideId": "warsawPact", "phaseId": "reserve" },
            "units": [
                { "id": BRIGADE, "hex": "2617" },
                { "id": "soviet.2gta.21motorRifleDivision", "hex": "2516" },
                { "id": "soviet.2gta.16guardsTankDivision", "hex": "2519" },
                { "id": "soviet.2gta.94guardsMotorRifleDivision", "hex": "2817" }
            ]
        }),
    );
    assert_eq!(phase(&started), "reserve");
    assert_eq!(started["snapshot"]["units"].as_array().unwrap().len(), 4);
    assert_eq!(unit(&started, BRIGADE)["supply"]["movement"], "supplied");

    let mut revision = 0;
    let nato_turn = accepted(&mut engine, &mut revision, json!({ "type": "endPhase" }));
    assert_eq!(phase(&nato_turn), "battlePlanning");
    let cut_off = nato_turn["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["type"] == "unitSupplyChanged")
        .expect("supply change event");
    assert_eq!(cut_off["unitId"], BRIGADE);
    assert_eq!(cut_off["supply"]["combat"], "outOfSupply");
    assert_eq!(
        unit(&nato_turn, BRIGADE)["supply"]["movement"],
        "outOfSupply"
    );

    // Out of Movement Supply, the brigade may not March out of the pocket.
    let march = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": revision,
        "command": { "type": "moveUnit", "unitId": BRIGADE, "destination": "2717", "mode": "march" }
    }));
    assert_eq!(march["code"], "marchUnavailable");
}

/// Malformed setups are rejected with a reason instead of starting a game.
#[test]
fn invalid_setups_are_rejected() {
    let mut engine = EngineProcess::start();
    let new_game = |setup: Value| json!({ "type": "newGame", "scenarioId": "nato-baltap-1983", "gameId": "bad-setup", "setup": setup });
    let unknown_unit = engine.send(new_game(
        json!({ "units": [{ "id": "nobody", "hex": "2617" }] }),
    ));
    assert_eq!(unknown_unit["code"], "invalidSetup");
    let unreachable = engine.send(new_game(
        json!({ "start": { "gameTurn": 9, "sideId": "nato", "phaseId": "combat" } }),
    ));
    assert_eq!(unreachable["code"], "invalidSetup");
    let typo = engine.send(new_game(json!({ "unit": [] })));
    assert_eq!(typo["code"], "invalidRequest");
    // No game was started by the rejected requests.
    let snapshot = engine.send(json!({ "type": "getSnapshot" }));
    assert_eq!(snapshot["code"], "gameNotStarted");
}
