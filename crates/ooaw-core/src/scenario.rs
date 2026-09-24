use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SideId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhaseId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SideDefinition {
    pub id: SideId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PhaseActor {
    All,
    Side { side_id: SideId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhaseExecution {
    Interactive,
    Conditional,
    Automatic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseDefinition {
    pub id: StepId,
    pub phase_id: PhaseId,
    pub actor: PhaseActor,
    pub execution: PhaseExecution,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioDefinition {
    pub id: String,
    pub name: String,
    pub max_game_turns: u16,
    pub sides: Vec<SideDefinition>,
    pub turn_sequence: Vec<PhaseDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioSummary {
    pub id: String,
    pub name: String,
    pub max_game_turns: u16,
    pub sides: Vec<SideDefinition>,
}

impl From<&ScenarioDefinition> for ScenarioSummary {
    fn from(value: &ScenarioDefinition) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            max_game_turns: value.max_game_turns,
            sides: value.sides.clone(),
        }
    }
}

fn side(id: &str, name: &str) -> SideDefinition {
    SideDefinition {
        id: SideId(id.to_owned()),
        name: name.to_owned(),
    }
}

fn step(side_id: &str, phase_id: &str, execution: PhaseExecution) -> PhaseDefinition {
    PhaseDefinition {
        id: StepId(format!("{side_id}.{phase_id}")),
        phase_id: PhaseId(phase_id.to_owned()),
        actor: PhaseActor::Side {
            side_id: SideId(side_id.to_owned()),
        },
        execution,
    }
}

fn joint_step(phase_id: &str, execution: PhaseExecution) -> PhaseDefinition {
    PhaseDefinition {
        id: StepId(format!("joint.{phase_id}")),
        phase_id: PhaseId(phase_id.to_owned()),
        actor: PhaseActor::All,
        execution,
    }
}

fn simplified_side_turn(side_id: &str) -> Vec<PhaseDefinition> {
    vec![
        step(side_id, "preBattle", PhaseExecution::Interactive),
        step(side_id, "battlePlanning", PhaseExecution::Interactive),
        step(side_id, "offensiveStrike", PhaseExecution::Interactive),
        step(side_id, "combat", PhaseExecution::Interactive),
        step(side_id, "reserve", PhaseExecution::Interactive),
        step(side_id, "postBattle", PhaseExecution::Automatic),
    ]
}

/// Returns a fresh scenario definition from the built-in registry.
///
/// The current entry deliberately contains only the turn framework. Unit setup,
/// reinforcements, resources and victory conditions will be added with their
/// corresponding rules.
pub fn find_scenario(id: &str) -> Option<ScenarioDefinition> {
    match id {
        "nato-1983-standard" => {
            let mut turn_sequence = vec![
                joint_step("jointStatus", PhaseExecution::Interactive),
                joint_step("jointReinforcement", PhaseExecution::Interactive),
            ];
            turn_sequence.extend(simplified_side_turn("warsawPact"));
            turn_sequence.extend(simplified_side_turn("nato"));
            Some(ScenarioDefinition {
                id: id.to_owned(),
                name: "NATO 1983 Rules Prototype".to_owned(),
                max_game_turns: 14,
                sides: vec![side("warsawPact", "Warsaw Pact"), side("nato", "NATO")],
                turn_sequence,
            })
        }
        _ => None,
    }
}
