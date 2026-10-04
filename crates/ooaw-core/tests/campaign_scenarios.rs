//! Campaign scenarios through the engine process's JSON protocol.

use std::collections::HashSet;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

const CAMPAIGNS: [&str; 6] = [
    "nato-strategic-surprise-1983",
    "nato-strategic-surprise-1988",
    "nato-extended-buildup-1983",
    "nato-extended-buildup-1988",
    "nato-war-of-nerves-1983",
    "nato-war-of-nerves-1988",
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
    for id in CAMPAIGNS {
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
    for (index, id) in CAMPAIGNS.iter().enumerate() {
        assert!(ids.contains(id));
        let response = &responses[index + 1];
        assert_eq!(response["type"], "gameStarted");
        assert_eq!(response["snapshot"]["scenario"]["id"], *id);
        assert_eq!(response["snapshot"]["scenario"]["maxGameTurns"], 14);
        assert_eq!(response["map"]["id"], "nato-central-europe");
    }
}
