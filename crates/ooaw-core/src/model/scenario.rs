use serde::{Deserialize, Serialize};

use super::map::nato_map;
use super::scenario_baltap::baltap_scenario;
use super::{
    BattlePlanningRules, FormationId, HexId, MapDefinition, NationId, PhaseActor, PhaseDefinition,
    PhaseExecution, PhaseId, SideDefinition, SideId, StepId, UnitDefinition, UnitId, UnitLocation,
    UnitState, UnitStepDefinition, UnitSupplyState, UnitTraitId, UnitTypeId,
};

/// Static rules and content currently used to create a game scenario.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioDefinition {
    /// Stable identifier accepted by [`find_scenario`].
    pub id: String,
    /// Human-readable scenario title.
    pub name: String,
    /// Number of game turns after which the scenario ends.
    pub max_game_turns: u16,
    /// Authoritative map used by the scenario.
    pub map: MapDefinition,
    /// Scenario-selected rules for movement and planning actions.
    pub battle_planning_rules: BattlePlanningRules,
    /// Sides that participate in the scenario.
    pub sides: Vec<SideDefinition>,
    /// Ordered sequence repeated during every game turn.
    pub turn_sequence: Vec<PhaseDefinition>,
    /// Opening units and later arrivals scheduled by game turn.
    pub reinforcements: Vec<ReinforcementDefinition>,
    /// HQs that may give Offensive Support, with their Subordinate formations (25.4).
    pub offensive_support_hqs: Vec<OffensiveSupportHq>,
    /// Units the scenario removes from play at the start of a game turn.
    pub withdrawals: Vec<Withdrawal>,
}

/// An HQ able to provide Offensive Support and the formations it commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OffensiveSupportHq {
    /// The HQ unit; its first step's Attack value is its Support Range (3.4.1).
    pub hq_id: UnitId,
    /// Formations whose units are Subordinate to the HQ.
    pub formations: Vec<FormationId>,
}

/// A unit removed from play by the scenario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Withdrawal {
    /// Game turn at whose Joint Reinforcement Phase the unit leaves play.
    pub game_turn: u16,
    /// Unit removed.
    pub unit_id: UnitId,
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
    /// Stable identifier of the authoritative map selected by the scenario.
    pub map_id: String,
    /// Sides available in the scenario.
    pub sides: Vec<SideDefinition>,
}

impl From<&ScenarioDefinition> for ScenarioSummary {
    /// Copies client-facing metadata from a full scenario definition.
    ///
    /// # Parameters
    ///
    /// - `value`: Source scenario whose identifier, title, turn limit, map ID,
    ///   and participating sides are copied into the summary.
    ///
    /// # Returns
    ///
    /// An owned summary without the scenario's map contents, rules, or unit
    /// deployment schedule. The source definition remains available to the caller.
    fn from(value: &ScenarioDefinition) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            max_game_turns: value.max_game_turns,
            map_id: value.map.id.clone(),
            sides: value.sides.clone(),
        }
    }
}

/// Builds a side definition from its stable identifier and display name.
pub(crate) fn side(id: &str, name: &str) -> SideDefinition {
    SideDefinition {
        id: SideId(id.to_owned()),
        name: name.to_owned(),
    }
}

/// Builds a phase step for one side with a side-qualified identifier and execution mode.
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

/// Builds a phase step shared by all sides with a joint-qualified identifier.
fn joint_step(phase_id: &str, execution: PhaseExecution) -> PhaseDefinition {
    PhaseDefinition {
        id: StepId(format!("joint.{phase_id}")),
        phase_id: PhaseId(phase_id.to_owned()),
        actor: PhaseActor::All,
        execution,
    }
}

/// Builds one side's ordered phases from pre-battle supply through post-battle cleanup.
fn simplified_side_turn(side_id: &str) -> Vec<PhaseDefinition> {
    vec![
        step(side_id, "preBattle", PhaseExecution::Automatic),
        step(side_id, "battlePlanning", PhaseExecution::Interactive),
        step(side_id, "offensiveStrike", PhaseExecution::Interactive),
        step(side_id, "combat", PhaseExecution::Interactive),
        step(side_id, "reserve", PhaseExecution::Interactive),
        step(side_id, "postBattle", PhaseExecution::Automatic),
    ]
}

/// Builds the joint opening phases followed by Warsaw Pact and NATO player turns.
pub(crate) fn standard_turn_sequence() -> Vec<PhaseDefinition> {
    let mut turn_sequence = vec![
        joint_step("jointStatus", PhaseExecution::Interactive),
        joint_step("jointReinforcement", PhaseExecution::Automatic),
    ];
    turn_sequence.extend(simplified_side_turn("warsawPact"));
    turn_sequence.extend(simplified_side_turn("nato"));
    turn_sequence
}

/// Builds a fully supplied scheduled unit, deriving hard and air-transport traits from its type.
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
    let supply = if unit_type_id == "headquarters" || traits.contains(&"headquarters") {
        UnitSupplyState::supplied_headquarters()
    } else {
        UnitSupplyState::supplied_combat_unit()
    };

    let mut traits: Vec<_> = traits
        .iter()
        .map(|id| UnitTraitId((*id).to_owned()))
        .collect();
    let type_key = unit_type_id.to_ascii_lowercase();
    if ["tank", "mechanized", "armored", "motorrifle"]
        .iter()
        .any(|key| type_key.contains(key))
        && !traits.iter().any(|item| item.0 == "hard")
    {
        traits.push(UnitTraitId("hard".to_owned()));
    }
    if ["airborne", "airmobile"]
        .iter()
        .any(|key| type_key.contains(key))
        && !traits.iter().any(|item| item.0 == "airTransportable")
    {
        traits.push(UnitTraitId("airTransportable".to_owned()));
    }

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
                traits,
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
            supply,
            train_status: None,
            disruption: None,
        },
    }
}

/// Returns a fresh scenario definition from the built-in registry.
///
/// Built-in entries contain their map, rules, turn sequence, opening deployment,
/// and reinforcement schedule. Each call returns owned data that can be adjusted
/// before creating a game.
///
/// # Parameters
///
/// - `id`: Exact, case-sensitive registry identifier, such as
///   `nato-baltap-1983` or `nato-1983-standard`.
///
/// # Returns
///
/// `Some(definition)` for a registered scenario, or `None` for an unknown ID.
/// Available identifiers can be discovered with [`list_scenarios`].
pub fn find_scenario(id: &str) -> Option<ScenarioDefinition> {
    match id {
        "nato-baltap-1983" => Some(baltap_scenario()),
        "nato-1983-standard" => Some(ScenarioDefinition {
            id: id.to_owned(),
            name: "NATO 1983 Rules Prototype".to_owned(),
            max_game_turns: 14,
            map: nato_map(),
            battle_planning_rules: BattlePlanningRules::nato_standard(),
            sides: vec![side("warsawPact", "Warsaw Pact"), side("nato", "NATO")],
            turn_sequence: standard_turn_sequence(),
            offensive_support_hqs: Vec::new(),
            withdrawals: Vec::new(),
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

/// Returns client-facing summaries for all scenarios in the built-in registry.
///
/// # Returns
///
/// Owned metadata in registry order, including each scenario's identifier,
/// title, turn limit, map identifier, and sides. Full setup and rules can be
/// loaded by passing a summary's identifier to [`find_scenario`].
pub fn list_scenarios() -> Vec<ScenarioSummary> {
    ["nato-baltap-1983", "nato-1983-standard"]
        .into_iter()
        .filter_map(find_scenario)
        .map(|scenario| ScenarioSummary::from(&scenario))
        .collect()
}
