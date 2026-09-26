use serde::{Deserialize, Serialize};

use super::scenario_baltap::baltap_scenario;
use super::{
    FormationId, HexId, NationId, PhaseActor, PhaseDefinition, PhaseExecution, PhaseId,
    SideDefinition, SideId, StepId, UnitDefinition, UnitId, UnitLocation, UnitState,
    UnitStepDefinition, UnitTraitId, UnitTypeId,
};

/// Static rules and content currently used to create a game scenario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioDefinition {
    /// Stable identifier accepted by [`find_scenario`].
    pub id: String,
    /// Human-readable scenario title.
    pub name: String,
    /// Number of game turns after which the scenario ends.
    pub max_game_turns: u16,
    /// Sides that participate in the scenario.
    pub sides: Vec<SideDefinition>,
    /// Ordered sequence repeated during every game turn.
    pub turn_sequence: Vec<PhaseDefinition>,
    /// Opening units and later arrivals scheduled by game turn.
    pub reinforcements: Vec<ReinforcementDefinition>,
}

/// A unit scheduled to enter authoritative state on a specific game turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReinforcementDefinition {
    /// One-based game turn on which the unit arrives.
    pub game_turn: u16,
    /// Complete initial state assigned to the arriving unit.
    pub unit: UnitState,
}

/// Client-facing scenario metadata that omits internal setup details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioSummary {
    /// Stable identifier accepted when creating a game.
    pub id: String,
    /// Human-readable scenario title.
    pub name: String,
    /// Number of game turns in the scenario.
    pub max_game_turns: u16,
    /// Sides available in the scenario.
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

pub(crate) fn side(id: &str, name: &str) -> SideDefinition {
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

pub(crate) fn standard_turn_sequence() -> Vec<PhaseDefinition> {
    let mut turn_sequence = vec![
        joint_step("jointStatus", PhaseExecution::Interactive),
        joint_step("jointReinforcement", PhaseExecution::Automatic),
    ];
    turn_sequence.extend(simplified_side_turn("warsawPact"));
    turn_sequence.extend(simplified_side_turn("nato"));
    turn_sequence
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn reinforcement(
    game_turn: u16,
    id: &str,
    name: &str,
    side_id: &str,
    nation_id: &str,
    unit_type_id: &str,
    formation_id: Option<&str>,
    location: UnitLocation,
    traits: &[&str],
    steps: &[(u16, u16, u16)],
) -> ReinforcementDefinition {
    ReinforcementDefinition {
        game_turn,
        unit: UnitState {
            definition: UnitDefinition {
                id: UnitId(id.to_owned()),
                name: name.to_owned(),
                side_id: SideId(side_id.to_owned()),
                nation_id: NationId(nation_id.to_owned()),
                unit_type_id: UnitTypeId(unit_type_id.to_owned()),
                formation_id: formation_id.map(|id| FormationId(id.to_owned())),
                traits: traits
                    .iter()
                    .map(|id| UnitTraitId((*id).to_owned()))
                    .collect(),
                steps: steps
                    .iter()
                    .map(|&(attack, defense, movement)| UnitStepDefinition {
                        attack,
                        defense,
                        movement,
                    })
                    .collect(),
            },
            strength_step_index: 0,
            location,
        },
    }
}

/// Returns a fresh scenario definition from the built-in registry.
///
/// Built-in entries contain their turn framework, opening deployment, and
/// reinforcement schedule.
pub fn find_scenario(id: &str) -> Option<ScenarioDefinition> {
    match id {
        "nato-baltap-1983" => Some(baltap_scenario()),
        "nato-1983-standard" => Some(ScenarioDefinition {
            id: id.to_owned(),
            name: "NATO 1983 Rules Prototype".to_owned(),
            max_game_turns: 14,
            sides: vec![side("warsawPact", "Warsaw Pact"), side("nato", "NATO")],
            turn_sequence: standard_turn_sequence(),
            reinforcements: vec![
                reinforcement(
                    1,
                    "soviet.6thGuardsMotorRifleDivision",
                    "6th Guards Motor Rifle Division",
                    "warsawPact",
                    "sovietUnion",
                    "motorRifleDivision",
                    None,
                    UnitLocation::Hex {
                        hex_id: HexId("2806".to_owned()),
                    },
                    &[],
                    &[(8, 6, 5), (4, 4, 5)],
                ),
                reinforcement(
                    1,
                    "westGermany.1stBrigade.1stPanzerDivision",
                    "1st Brigade, 1st Panzer Division",
                    "nato",
                    "westGermany",
                    "armoredBrigade",
                    Some("westGermany.1stPanzerDivision"),
                    UnitLocation::Hex {
                        hex_id: HexId("3216".to_owned()),
                    },
                    &[],
                    &[(3, 3, 6)],
                ),
            ],
        }),
        _ => None,
    }
}

/// Returns summaries for every scenario that can be passed to `find_scenario`.
pub fn list_scenarios() -> Vec<ScenarioSummary> {
    ["nato-baltap-1983", "nato-1983-standard"]
        .into_iter()
        .filter_map(find_scenario)
        .map(|scenario| ScenarioSummary::from(&scenario))
        .collect()
}
