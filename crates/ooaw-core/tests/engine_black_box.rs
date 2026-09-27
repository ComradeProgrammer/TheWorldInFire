use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

struct EngineProcess {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl EngineProcess {
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
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn event_types(response: &Value) -> Vec<&str> {
    response["events"]
        .as_array()
        .expect("response has events")
        .iter()
        .map(|event| event["type"].as_str().expect("event has a type"))
        .collect()
}

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

    let started = engine.send(json!({
        "type": "newGame",
        "scenarioId": "nato-1983-standard",
        "gameId": "black-box-e2e"
    }));
    assert_eq!(started["type"], "gameStarted");
    assert_eq!(started["snapshot"]["revision"], 0);
    assert_eq!(
        started["snapshot"]["turn"]["currentStep"]["phaseId"],
        "jointStatus"
    );
    assert_eq!(started["snapshot"]["units"], json!([]));
    assert_eq!(started["map"]["id"], "nato-central-europe");

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
    assert_eq!(rejected["code"], "wrongPhase");

    let opening = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": 0,
        "command": { "type": "endPhase" }
    }));
    assert_eq!(opening["type"], "commandAccepted");
    assert_eq!(opening["revision"], 1);
    assert_eq!(opening["snapshot"]["revision"], 1);
    assert_eq!(opening["snapshot"]["units"].as_array().unwrap().len(), 2);
    assert_eq!(
        opening["snapshot"]["turn"]["currentStep"]["phaseId"],
        "battlePlanning"
    );
    assert_eq!(
        event_types(&opening),
        vec![
            "phaseEnded",
            "phaseStarted",
            "airPointsReset",
            "reinforcementsArrived",
            "phaseEnded",
            "phaseStarted",
            "preBattleSupplyChecked",
            "phaseEnded",
            "phaseStarted",
        ]
    );

    let planned = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": 1,
        "command": {
            "type": "setResupplyTarget",
            "unitId": "soviet.6thGuardsMotorRifleDivision"
        }
    }));
    assert_eq!(planned["type"], "commandAccepted");
    assert_eq!(planned["revision"], 2);
    assert_eq!(event_types(&planned), vec!["resupplyTargetSet"]);
    assert_eq!(
        planned["snapshot"]["battlePlan"]["resupplyTargetUnitId"],
        "soviet.6thGuardsMotorRifleDivision"
    );

    let stale = engine.send(json!({
        "type": "submitCommand",
        "expectedRevision": 1,
        "command": { "type": "endPhase" }
    }));
    assert_eq!(stale["type"], "error");
    assert_eq!(stale["code"], "revisionMismatch");

    let snapshot = engine.send(json!({ "type": "getSnapshot" }));
    assert_eq!(snapshot["type"], "snapshot");
    assert_eq!(snapshot["snapshot"]["revision"], 2);
    assert_eq!(
        snapshot["snapshot"]["battlePlan"]["resupplyTargetUnitId"],
        "soviet.6thGuardsMotorRifleDivision"
    );
}
