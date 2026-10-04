use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use super::map::nato_map;
use super::scenario::{reinforcement, side, standard_turn_sequence};
use super::{
    BattlePlanningRules, FormationId, HexId, OffensiveSupportHq, ScenarioDefinition, TrainStatus,
    UnitId, UnitLocation,
};

/// Stable public IDs, shared by lookup and the client-facing registry.
pub(crate) const CAMPAIGN_IDS: [&str; 6] = [
    "nato-strategic-surprise-1983",
    "nato-strategic-surprise-1988",
    "nato-extended-buildup-1983",
    "nato-extended-buildup-1988",
    "nato-war-of-nerves-1983",
    "nato-war-of-nerves-1988",
];

#[derive(Deserialize)]
struct CampaignData {
    units: BTreeMap<String, CampaignUnit>,
    scenarios: BTreeMap<String, CampaignSetup>,
}

#[derive(Deserialize)]
struct CampaignSetup {
    units: Vec<Deployment>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Deployment {
    unit: String,
    game_turn: u16,
    hex: Option<String>,
    /// Rail reinforcements arrive Entrained (13.6).
    #[serde(default)]
    entrained: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CampaignUnit {
    id: String,
    name: String,
    nation_id: String,
    unit_type_id: String,
    formation_id: String,
    traits: Vec<String>,
    steps: Vec<(u16, u16, u16)>,
}

fn campaign_data() -> &'static CampaignData {
    static DATA: OnceLock<CampaignData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../data/natoCampaigns.json"))
            .expect("embedded NATO campaign data must be valid")
    })
}

/// Builds the supported war-turn portion of one campaign setup.
///
/// These entries use the existing combat engine: Strategic Surprise's GT0 and
/// War of Nerves' peace/mobilization sequence are not implemented. Reforger units
/// remain in Strategic Reserve rather than acting as combat-ready sites. Data
/// retains later mobilization and optional intervention units for future rules,
/// but they are not scheduled into the current fourteen-war-turn game.
pub(crate) fn campaign_scenario(id: &str) -> Option<ScenarioDefinition> {
    if !CAMPAIGN_IDS.contains(&id) {
        return None;
    }
    let data = campaign_data();
    let setup = data.scenarios.get(id)?;
    let extended_buildup = id.starts_with("nato-extended-buildup-");
    let title = if extended_buildup {
        "Extended Buildup"
    } else if id.starts_with("nato-strategic-surprise-") {
        "Strategic Surprise"
    } else {
        "War of Nerves"
    };
    let year = id.rsplit('-').next()?;
    let mut rules = BattlePlanningRules {
        // 37.2.1.2 / 38.2.1.2 / 40.2.1.2.
        warsaw_pact_airlift_commands: if extended_buildup { 4 } else { 3 },
        nato_airlift_commands: if extended_buildup { 3 } else { 2 },
        ..BattlePlanningRules::nato_standard()
    };
    // 37.4.1.2: Surprise on the first war turn. WoN Surprise depends on NATO's
    // Alert Level (40.4.1.2); do not assume Surprise without that system.
    rules.air_power.surprise_turn = id.starts_with("nato-strategic-surprise-").then_some(1);
    let reinforcements: Vec<_> = setup
        .units
        .iter()
        .map(|deployment| {
            let unit = data
                .units
                .get(&deployment.unit)
                .expect("campaign deployment must reference a catalog unit");
            let side_id = if matches!(
                unit.nation_id.as_str(),
                "sovietUnion" | "eastGermany" | "czechoslovakia" | "poland"
            ) {
                "warsawPact"
            } else {
                "nato"
            };
            let location = match &deployment.hex {
                Some(id) => UnitLocation::Hex {
                    hex_id: HexId(id.clone()),
                },
                None => UnitLocation::StrategicReserve,
            };
            let traits: Vec<_> = unit.traits.iter().map(String::as_str).collect();
            let mut arrival = reinforcement(
                deployment.game_turn,
                &unit.id,
                &unit.name,
                side_id,
                &unit.nation_id,
                &unit.unit_type_id,
                Some(&unit.formation_id),
                location,
                &traits,
                &unit.steps,
            );
            if deployment.entrained {
                arrival.unit.train_status = Some(TrainStatus::Entrained);
            }
            arrival
        })
        .collect();
    // 25.4: WP Front HQs and U.S. Corps give Offensive Support. BAF and U.S.
    // III participate only in Extended Buildup. BC is never eligible.
    let offensive_support_hqs = reinforcements
        .iter()
        .filter_map(|arrival| {
            let unit = &arrival.unit.definition;
            let regular = matches!(
                unit.id.0.as_str(),
                "sovietUnion.negf"
                    | "sovietUnion.segf"
                    | "sovietUnion.czf"
                    | "sovietUnion.pof"
                    | "sovietUnion.bef"
                    | "sovietUnion.caf"
                    | "unitedStates.usv"
                    | "unitedStates.usvii"
            );
            let extended_only = extended_buildup
                && matches!(unit.id.0.as_str(), "sovietUnion.baf" | "unitedStates.usiii");
            if regular || extended_only {
                Some(OffensiveSupportHq {
                    hq_id: UnitId(unit.id.0.clone()),
                    formations: vec![FormationId(unit.formation_id.as_ref().unwrap().0.clone())],
                })
            } else {
                None
            }
        })
        .collect();
    Some(ScenarioDefinition {
        id: id.to_owned(),
        name: format!("{title} {year}"),
        max_game_turns: 14,
        map: nato_map(),
        battle_planning_rules: rules,
        sides: vec![side("warsawPact", "Warsaw Pact"), side("nato", "NATO")],
        turn_sequence: standard_turn_sequence(),
        reinforcements,
        offensive_support_hqs,
        withdrawals: Vec::new(),
    })
}
