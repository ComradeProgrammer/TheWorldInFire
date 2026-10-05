use super::{
    AirBaseDefinition, AirBaseId, AirUnitDefinition, AirUnitId, AirUnitKind, AirUnitStep, HexId,
    NationId, SideId,
};

fn step(
    air_combat: i8,
    evasion: i8,
    strike: i8,
    radius: u8,
    aew: i8,
    aew_radius: u8,
) -> AirUnitStep {
    AirUnitStep {
        air_combat,
        evasion,
        strike_modifier: strike,
        combat_radius: radius,
        aew_modifier: aew,
        aew_radius,
    }
}

fn unit(
    id: &str,
    name: &str,
    affiliation: (&str, &str),
    kind: AirUnitKind,
    base: &str,
    steps: [AirUnitStep; 2],
) -> AirUnitDefinition {
    AirUnitDefinition {
        id: AirUnitId(id.to_owned()),
        name: name.to_owned(),
        side_id: SideId(affiliation.0.to_owned()),
        nation_id: NationId(affiliation.1.to_owned()),
        kind,
        base_id: AirBaseId(base.to_owned()),
        steps,
    }
}

/// Initial playable air order of battle. Names follow 1985's squadron-and-aircraft style;
/// values are deliberately compact prototype values pending a full counter transcription.
pub(crate) fn standard_air_forces(year: &str) -> (Vec<AirBaseDefinition>, Vec<AirUnitDefinition>) {
    let bases = vec![
        AirBaseDefinition {
            id: AirBaseId("nato.airbase.west".to_owned()),
            name: "NATO Western Airbases".to_owned(),
            side_id: SideId("nato".to_owned()),
            anchor_hex_id: HexId("3534".to_owned()),
            sortie_capacity: 5,
            strike_modifier: -1,
        },
        AirBaseDefinition {
            id: AirBaseId("warsawPact.airbase.east".to_owned()),
            name: "Warsaw Pact Eastern Airbases".to_owned(),
            side_id: SideId("warsawPact".to_owned()),
            anchor_hex_id: HexId("3301".to_owned()),
            sortie_capacity: 5,
            strike_modifier: -1,
        },
    ];
    let nato_strike_name = if year == "1988" {
        "US 480th TFS — F-16C"
    } else {
        "US 480th TFS — F-4E"
    };
    let wp_fighter_name = if year == "1988" {
        "Soviet 787th IAP — MiG-29"
    } else {
        "Soviet 787th IAP — MiG-23ML"
    };
    let wp_aew_name = if year == "1988" {
        "Soviet AEW Detachment — A-50"
    } else {
        "Soviet AEW Detachment — Tu-126"
    };
    let units = vec![
        unit(
            "us.525.f15",
            "US 525th TFS — F-15C",
            ("nato", "unitedStates"),
            AirUnitKind::Fighter,
            "nato.airbase.west",
            [step(7, 6, 0, 4, 0, 0), step(6, 5, 0, 3, 0, 0)],
        ),
        unit(
            "westGermany.jg71.f4f",
            "WG JG 71 — F-4F",
            ("nato", "westGermany"),
            AirUnitKind::Fighter,
            "nato.airbase.west",
            [step(6, 5, 0, 4, 0, 0), step(5, 4, 0, 3, 0, 0)],
        ),
        unit(
            "us.480.strike",
            nato_strike_name,
            ("nato", "unitedStates"),
            AirUnitKind::FighterBomber,
            "nato.airbase.west",
            [step(5, 5, 1, 0, 0, 0), step(4, 4, 0, 0, 0, 0)],
        ),
        unit(
            "unitedKingdom.31.tornado",
            "UK No. 31 Squadron — Tornado GR.1",
            ("nato", "unitedKingdom"),
            AirUnitKind::FighterBomber,
            "nato.airbase.west",
            [step(4, 5, 1, 0, 0, 0), step(3, 4, 0, 0, 0, 0)],
        ),
        unit(
            "nato.e3a",
            "NATO AEW Force — E-3A Sentry",
            ("nato", "nato"),
            AirUnitKind::Aew,
            "nato.airbase.west",
            [step(0, 6, 0, 0, 2, 6), step(0, 5, 0, 0, 1, 5)],
        ),
        unit(
            "soviet.787.fighter",
            wp_fighter_name,
            ("warsawPact", "sovietUnion"),
            AirUnitKind::Fighter,
            "warsawPact.airbase.east",
            [step(6, 5, 0, 4, 0, 0), step(5, 4, 0, 3, 0, 0)],
        ),
        unit(
            "eastGermany.jg3.mig21",
            "East German JG-3 — MiG-21bis",
            ("warsawPact", "eastGermany"),
            AirUnitKind::Fighter,
            "warsawPact.airbase.east",
            [step(5, 4, 0, 3, 0, 0), step(4, 3, 0, 2, 0, 0)],
        ),
        unit(
            "soviet.296.su17",
            "Soviet 296th APIB — Su-17M",
            ("warsawPact", "sovietUnion"),
            AirUnitKind::FighterBomber,
            "warsawPact.airbase.east",
            [step(4, 4, 1, 0, 0, 0), step(3, 3, 0, 0, 0, 0)],
        ),
        unit(
            "soviet.559.su17",
            "Soviet 559th APIB — Su-17M",
            ("warsawPact", "sovietUnion"),
            AirUnitKind::FighterBomber,
            "warsawPact.airbase.east",
            [step(4, 4, 1, 0, 0, 0), step(3, 3, 0, 0, 0, 0)],
        ),
        unit(
            "soviet.aew",
            wp_aew_name,
            ("warsawPact", "sovietUnion"),
            AirUnitKind::Aew,
            "warsawPact.airbase.east",
            [step(0, 5, 0, 0, 1, 5), step(0, 4, 0, 0, 1, 4)],
        ),
    ];
    (bases, units)
}
