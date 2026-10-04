//! Simplified supply (a house rule replacing rule 10).
//!
//! Only the two alliances matter; nationality, home countries, ports, map
//! edges, and the supply of cities themselves are ignored.
//!
//! - A Line of Supply (LOS) is a path of hexes that never enters enemy units,
//!   enemy-controlled cities, All-Sea hexes or hexsides, or an EZOC hex without
//!   a friendly unit or friendly Free City in it (10.2.3).
//! - An HQ is supplied if it traces a LOS of up to ten hexes to any city its
//!   side controls, Free or Conquered.
//! - A combat unit is supplied if it traces a LOS of up to ten hexes to a
//!   friendly Free City, or to a supplied friendly HQ within that HQ's Support
//!   Range. An HQ under a train marker supplies no one (10.4.4.7).
//! - West Berlin, an enclave, supplies only units in or adjacent to it.
//!
//! Supply is checked for the acting side at the start of its Player Turn and
//! sets both Movement and Combat Supply.

use std::collections::{HashMap, VecDeque};

use crate::event::{GameEvent, UnitSupplyCheck};
use crate::model::{
    HexsideFeature, SideId, SupplyStatus, Terrain, UnitId, UnitLocation, UnitState, UnitSupplyState,
};
use crate::movement::MovementContext;
use crate::rules::Rules;

/// Longest LOS to a city, in hexes.
const CITY_SUPPLY_RANGE: u16 = 10;

/// Hexes reachable from `from` by a LOS of at most `range` hexes, with their
/// distances. The starting hex counts as zero; the destination hex counts.
/// `ferry` allows crossing the Danish Ferry hexside.
pub(crate) fn los_distances(
    context: &MovementContext,
    from: &str,
    range: u16,
    ferry: bool,
) -> HashMap<String, u16> {
    let mut distances = HashMap::from([(from.to_owned(), 0_u16)]);
    let mut queue = VecDeque::from([from.to_owned()]);
    while let Some(current) = queue.pop_front() {
        let distance = distances[&current];
        if distance >= range {
            continue;
        }
        for next in context.neighbors(&current) {
            if distances.contains_key(next) {
                continue;
            }
            let features = context.side_features(&current, next);
            let blocked_side = if features.contains(&HexsideFeature::DanishFerry) {
                !ferry
            } else {
                context.prohibited_hexside(&current, next)
            };
            let hex = context.hexes[next];
            if blocked_side
                || hex.terrain == Terrain::Sea
                || context.enemy_occupied.contains(next)
                || context.enemy_free_cities.contains(next)
                || context.enemy_conquered_cities.contains(next)
                || (context.in_enemy_zoc(next) && !context.negates_ezoc_for_entry(next))
            {
                continue;
            }
            distances.insert(next.to_owned(), distance + 1);
            queue.push_back(next.to_owned());
        }
    }
    distances
}

/// An HQ's Support Range: the Attack value printed on its first step (3.4.1).
pub(crate) fn support_range(hq: &UnitState) -> u16 {
    hq.definition.steps.first().map_or(0, |step| step.attack)
}

fn status(supplied: bool) -> SupplyStatus {
    if supplied {
        SupplyStatus::Supplied
    } else {
        SupplyStatus::OutOfSupply
    }
}

impl Rules {
    /// Recomputes the supply of every unit of `side_id`, emitting a
    /// `UnitSupplyChanged` event for each unit whose status changed.
    pub(crate) fn update_supply(
        &mut self,
        side_id: &SideId,
        events: &mut Vec<GameEvent>,
    ) -> Vec<UnitSupplyCheck> {
        let mut checks = Vec::new();
        for (unit_id, ok) in self.trace_supply(side_id) {
            let unit = self.units.get_mut(&unit_id).expect("checked unit exists");
            let next = if unit.is_headquarters() {
                UnitSupplyState {
                    headquarters: Some(status(ok)),
                    movement: None,
                    combat: None,
                }
            } else {
                UnitSupplyState {
                    headquarters: None,
                    movement: Some(status(ok)),
                    combat: Some(status(ok)),
                }
            };
            if unit.supply != next {
                unit.supply = next.clone();
                events.push(GameEvent::UnitSupplyChanged {
                    unit_id: unit_id.clone(),
                    supply: next.clone(),
                });
            }
            checks.push(UnitSupplyCheck {
                unit_id,
                supply: next,
            });
        }
        checks
    }

    /// Whether each unit of `side_id` can trace supply now, in unit-ID order.
    fn trace_supply(&self, side_id: &SideId) -> Vec<(UnitId, bool)> {
        let context = MovementContext::new(self, side_id);
        // 10.2.4: only NATO traces across the Danish Ferry while Denmark stands.
        let ferry = side_id.0 == "nato";
        let hex_of = |unit: &UnitState| match &unit.location {
            UnitLocation::Hex { hex_id } => Some(hex_id.0.clone()),
            UnitLocation::StrategicReserve => None,
        };
        let city = |id: &str| context.hexes.get(id).and_then(|hex| hex.city.as_ref());
        let controlled = |id: &str| {
            self.city_control
                .iter()
                .any(|(hex_id, controller)| hex_id.0 == id && controller == side_id)
        };
        let side_units = || {
            self.units
                .values()
                .filter(move |unit| unit.definition.side_id == *side_id)
        };

        // HQs first: supplied HQs not under a train marker supply combat units.
        let mut results = Vec::new();
        let mut supplying_hqs: Vec<(String, u16)> = Vec::new();
        for unit in side_units().filter(|unit| unit.is_headquarters()) {
            let origin = hex_of(unit);
            let ok = origin.as_ref().is_none_or(|origin| {
                los_distances(&context, origin, CITY_SUPPLY_RANGE, ferry)
                    .keys()
                    .any(|id| controlled(id) && city(id).is_some_and(|city| !city.enclave))
            });
            if let (true, Some(origin), None) = (ok, origin, unit.train_status) {
                supplying_hqs.push((origin, support_range(unit)));
            }
            results.push((unit.id().clone(), ok));
        }

        let reach = supplying_hqs
            .iter()
            .map(|(_, range)| *range)
            .max()
            .unwrap_or(0)
            .max(CITY_SUPPLY_RANGE);
        for unit in side_units().filter(|unit| !unit.is_headquarters()) {
            let ok = hex_of(unit).is_none_or(|origin| {
                let distances = los_distances(&context, &origin, reach, ferry);
                let to_city = distances.iter().any(|(id, &distance)| {
                    context.friendly_free_cities.contains(id.as_str())
                        && city(id).is_some_and(|city| {
                            distance <= if city.enclave { 1 } else { CITY_SUPPLY_RANGE }
                        })
                });
                let to_hq = supplying_hqs.iter().any(|(hq_hex, range)| {
                    distances
                        .get(hq_hex)
                        .is_some_and(|distance| distance <= range)
                });
                to_city || to_hq
            });
            results.push((unit.id().clone(), ok));
        }
        results.sort_by(|a, b| a.0 .0.cmp(&b.0 .0));
        results
    }
}
