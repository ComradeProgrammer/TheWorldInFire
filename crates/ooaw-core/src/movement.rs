//! Movement rules shared by the `MoveUnit` command and movement previews.
//!
//! Rule references are to the NATO: The Cold War Goes Hot rulebook (2020).
//! Every legal destination offered by [`GameEngine::movement_options`] is found
//! by the same search and destination checks that validate `MoveUnit`.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::airspace::AirspaceMap;
use crate::engine::GameEngine;
use crate::error::RuleError;
use crate::model::{
    has_incompatible_movement, movement_spent, Airspace, HexId, HexsideFeature, MapHex,
    MovementMode, MovementModeOptions, MovementOption, PlannedMovement, SideId, SupplyStatus,
    Terrain, TrainStatus, UnitId, UnitLocation, UnitState,
};
use crate::reserve::MovementPhase;

/// Precomputed map and unit lookups for one movement query.
pub(crate) struct MovementContext<'a> {
    /// Map hexes indexed by printed identifier for this query.
    pub(crate) hexes: HashMap<&'a str, &'a MapHex>,
    /// Shared hexside features indexed in both endpoint directions.
    pub(crate) hexsides: HashMap<(&'a str, &'a str), &'a [HexsideFeature]>,
    /// Hexes subject to enemy unit or Free City zones of control.
    pub(crate) enemy_zoc: HashSet<String>,
    /// Map hexes containing units of another side.
    pub(crate) enemy_occupied: HashSet<&'a str>,
    /// Map hexes containing units of the moving side.
    pub(crate) friendly_occupied: HashSet<&'a str>,
    /// Enemy-controlled city hexes still held by their original alliance.
    pub(crate) enemy_free_cities: HashSet<&'a str>,
    /// Enemy-controlled city hexes held by an alliance other than their original owner.
    pub(crate) enemy_conquered_cities: HashSet<&'a str>,
    /// City hexes controlled by the moving side as their original owner.
    pub(crate) friendly_free_cities: HashSet<&'a str>,
    /// Airspace from the moving side's point of view.
    airspace: AirspaceMap,
    /// Hexes inside an enemy Air Interdiction Zone (23.8).
    enemy_interdiction: HashSet<String>,
    /// Breakthrough Zones: each Breakthrough Marker hex and its six neighbours (25.9).
    breakthrough_zone: HashSet<String>,
}

impl<'a> MovementContext<'a> {
    /// Caches map, occupation, city, airspace, interdiction, and breakthrough data for the moving side.
    pub(crate) fn new(state: &'a GameEngine, side_id: &SideId) -> Self {
        let hexes: HashMap<_, _> = state
            .scenario
            .map
            .hexes
            .iter()
            .map(|hex| (hex.id.0.as_str(), hex))
            .collect();
        let mut hexsides = HashMap::new();
        for side in &state.scenario.map.hexsides {
            hexsides.insert(
                (side.a.0.as_str(), side.b.0.as_str()),
                side.features.as_slice(),
            );
            hexsides.insert(
                (side.b.0.as_str(), side.a.0.as_str()),
                side.features.as_slice(),
            );
        }
        let mut context = Self {
            hexes,
            hexsides,
            enemy_zoc: HashSet::new(),
            enemy_occupied: HashSet::new(),
            friendly_occupied: HashSet::new(),
            enemy_free_cities: HashSet::new(),
            enemy_conquered_cities: HashSet::new(),
            friendly_free_cities: HashSet::new(),
            airspace: state.airspace_map(side_id),
            enemy_interdiction: HashSet::new(),
            breakthrough_zone: HashSet::new(),
        };
        for marker in &state.breakthrough_markers {
            context.breakthrough_zone.insert(marker.0.clone());
            for neighbor in context.neighbors(&marker.0) {
                context.breakthrough_zone.insert(neighbor.to_owned());
            }
        }
        for zone in state
            .air_interdiction_zones
            .iter()
            .filter(|zone| zone.side_id != *side_id)
        {
            context.enemy_interdiction.insert(zone.hex_id.0.clone());
            for neighbor in context.neighbors(&zone.hex_id.0) {
                context.enemy_interdiction.insert(neighbor.to_owned());
            }
        }
        for (hex_id, controller) in &state.city_control {
            let id = hex_id.0.as_str();
            let free = state.city_owner(hex_id) == Some(controller);
            match (controller == side_id, free) {
                (true, true) => {
                    context.friendly_free_cities.insert(id);
                }
                (true, false) => {}
                (false, true) => {
                    context.enemy_free_cities.insert(id);
                    // 30.2.2: a Free City exerts a ZOC in its own hex only.
                    context.enemy_zoc.insert(id.to_owned());
                }
                (false, false) => {
                    context.enemy_conquered_cities.insert(id);
                }
            }
        }
        for unit in state.units.values() {
            let UnitLocation::Hex { hex_id } = &unit.location else {
                continue;
            };
            if unit.definition.side_id == *side_id {
                context.friendly_occupied.insert(hex_id.0.as_str());
                continue;
            }
            context.enemy_occupied.insert(hex_id.0.as_str());
            // 13.4.1: units under a train marker have no ZOC.
            if unit.train_status.is_some() {
                continue;
            }
            // 8.0: HQs and units with an Attack Strength of 1+ project into the six
            // adjacent hexes; zero-attack units only into their own hex.
            context.enemy_zoc.insert(hex_id.0.clone());
            let projects =
                unit.is_headquarters() || unit.current_step().is_some_and(|step| step.attack > 0);
            if projects {
                for neighbor in context.neighbors(hex_id.0.as_str()) {
                    context.enemy_zoc.insert(neighbor.to_owned());
                }
            }
        }
        context
    }

    /// Adjacent on-map hexes (pointy-top rows, odd rows shifted right).
    pub(crate) fn neighbors(&self, id: &str) -> Vec<&'a str> {
        let Some(hex) = self.hexes.get(id) else {
            return Vec::new();
        };
        let row = i32::from(hex.row);
        let col = i32::from(hex.col);
        let diagonal = if row % 2 == 0 {
            [
                (row - 1, col),
                (row - 1, col + 1),
                (row + 1, col),
                (row + 1, col + 1),
            ]
        } else {
            [
                (row - 1, col - 1),
                (row - 1, col),
                (row + 1, col - 1),
                (row + 1, col),
            ]
        };
        [(row, col - 1), (row, col + 1)]
            .into_iter()
            .chain(diagonal)
            .filter(|(r, c)| *r >= 0 && *c >= 0)
            .filter_map(|(r, c)| {
                self.hexes
                    .get_key_value(format!("{r:02}{c:02}").as_str())
                    .map(|(key, _)| *key)
            })
            .collect()
    }

    /// Returns the features of the shared hexside, or an empty slice when none are recorded.
    pub(crate) fn side_features(&self, a: &str, b: &str) -> &'a [HexsideFeature] {
        self.hexsides.get(&(a, b)).copied().unwrap_or(&[])
    }

    /// 12.1.1: Blocked hexsides and All-Sea hexsides without a causeway.
    pub(crate) fn prohibited_hexside(&self, a: &str, b: &str) -> bool {
        let features = self.side_features(a, b);
        features.contains(&HexsideFeature::Blocked)
            || (features.contains(&HexsideFeature::AllSea)
                && !features.contains(&HexsideFeature::Causeway))
    }

    /// Checks whether the hex lies in the cached enemy zone of control.
    pub(crate) fn in_enemy_zoc(&self, id: &str) -> bool {
        self.enemy_zoc.contains(id)
    }

    /// Checks whether the hex has exclusively friendly airspace.
    fn friendly_airspace(&self, id: &str) -> bool {
        self.airspace.of(id) == Airspace::Friendly
    }

    /// Checks whether friendly occupation or a friendly Free City permits Soft-unit entry from another EZOC (8.5, 30.2.3).
    pub(crate) fn negates_ezoc_for_entry(&self, id: &str) -> bool {
        // 8.5 (5), 30.2.3: a friendly unit or friendly Free City in the hex lets a
        // Soft unit enter it directly from another EZOC hex.
        self.friendly_occupied.contains(id) || self.friendly_free_cities.contains(id)
    }
}

/// Shortest legal arrival cost and predecessor for every reachable hex.
struct Search {
    /// Printed identifier of the starting hex for the route search.
    origin: String,
    /// Lowest known arrival cost for each reachable hex, including the origin at zero.
    cost: HashMap<String, u16>,
    /// Predecessor hex for reconstructing each cheapest arrival route.
    previous: HashMap<String, String>,
}

impl Search {
    /// Reconstructs a destination's predecessor route, excluding the origin hex.
    fn path_to(&self, destination: &str) -> Vec<HexId> {
        let mut reversed = vec![HexId(destination.to_owned())];
        let mut current = destination;
        while let Some(parent) = self.previous.get(current) {
            if *parent == self.origin {
                break;
            }
            reversed.push(HexId(parent.clone()));
            current = parent;
        }
        reversed.reverse();
        reversed
    }
}

/// Result of a Tactical or March search.
struct GroundSearch {
    /// Cheapest ground arrival costs and predecessor routes within the movement allowance.
    search: Search,
    /// Movement points left for this order.
    remaining: u16,
    /// Legal single-hex moves from the origin and their cost, usable as
    /// Minimum movement (12.5) when the unit has not moved yet.
    minimum_moves: HashMap<String, u16>,
}

impl GroundSearch {
    /// Returns a ground route and cost, allowing minimum movement or reporting no legal route.
    fn route(&self, destination: &str) -> Result<(Vec<HexId>, u16), RuleError> {
        if let Some(&cost) = self.search.cost.get(destination) {
            return Ok((self.search.path_to(destination), cost));
        }
        if let Some(&direct) = self.minimum_moves.get(destination) {
            return Ok((vec![HexId(destination.to_owned())], direct));
        }
        Err(RuleError::new(
            "noLegalRoute",
            format!(
                "No legal ground route reaches that hex with {} movement points remaining",
                self.remaining
            ),
        ))
    }
}

/// Finds cheapest reachable hexes and predecessors using the supplied neighbor and legal-edge functions.
/// Result of a lift search: the hexes a flight or voyage can reach, with routes.
struct LiftSearch {
    /// Predecessor of each reached hex; `None` for a starting hex.
    previous: HashMap<String, Option<String>>,
    /// Whether the order starts on the map (its own hex is then left out of routes).
    from_map: bool,
}

impl LiftSearch {
    /// The route to `destination`, excluding a starting hex on the map.
    fn path(&self, destination: &str) -> Option<Vec<HexId>> {
        self.previous.get(destination)?;
        let mut reversed = vec![HexId(destination.to_owned())];
        let mut current = destination;
        while let Some(Some(parent)) = self.previous.get(current) {
            if self.previous.get(parent.as_str()) == Some(&None) && self.from_map {
                break;
            }
            reversed.push(HexId(parent.clone()));
            current = parent;
        }
        reversed.reverse();
        Some(reversed)
    }
}

fn dijkstra(
    origin: &str,
    edge: &dyn Fn(&str, &str, u16) -> Option<u16>,
    neighbors: &dyn Fn(&str) -> Vec<String>,
) -> Search {
    let mut search = Search {
        origin: origin.to_owned(),
        cost: HashMap::from([(origin.to_owned(), 0)]),
        previous: HashMap::new(),
    };
    let mut queue = BinaryHeap::from([Reverse((0_u16, origin.to_owned()))]);
    while let Some(Reverse((cost, current))) = queue.pop() {
        if search.cost.get(&current).is_some_and(|best| cost > *best) {
            continue;
        }
        for next in neighbors(&current) {
            let Some(step) = edge(&current, &next, cost) else {
                continue;
            };
            let next_cost = cost.saturating_add(step);
            if search.cost.get(&next).is_none_or(|best| next_cost < *best) {
                search.cost.insert(next.clone(), next_cost);
                search.previous.insert(next.clone(), current.clone());
                queue.push(Reverse((next_cost, next)));
            }
        }
    }
    search
}

impl GameEngine {
    /// Previews every movement system for a unit in a fixed order.
    ///
    /// Each entry contains legal destinations or a structured reason the system
    /// is unavailable. The order is Tactical, March, Rail, then Air Transport.
    /// Reserve Phase restrictions are included in these previews.
    ///
    /// # Parameters
    ///
    /// - `unit_id`: Identifier of an existing unit controlled by the acting side.
    ///
    /// # Returns
    ///
    /// One [`MovementModeOptions`] per system. Mode-specific rule failures are
    /// recorded in each entry's `unavailable` field rather than failing the list.
    ///
    /// # Errors
    ///
    /// Returns a [`RuleError`] if no valid movement phase is active, the game has
    /// ended, the unit is unknown, or the acting side does not control it.
    pub fn movement_modes(&self, unit_id: &UnitId) -> Result<Vec<MovementModeOptions>, RuleError> {
        let (side_id, _) = self.movement_phase()?;
        self.owned_unit(unit_id, &side_id)?;
        Ok([
            MovementMode::Tactical,
            MovementMode::March,
            MovementMode::Rail,
            MovementMode::AirTransport,
            MovementMode::Paradrop,
            MovementMode::SeaTransport,
        ]
        .into_iter()
        .map(|mode| match self.movement_options(unit_id, mode) {
            Ok(options) => MovementModeOptions {
                mode,
                unavailable: None,
                options,
            },
            Err(error) => MovementModeOptions {
                mode,
                unavailable: Some(error),
                options: Vec::new(),
            },
        })
        .collect())
    }

    /// Lists every hex a unit may legally reach with one movement order.
    ///
    /// The preview uses the same route search and destination checks as `MoveUnit`,
    /// including movement points, zones of control, terrain, transport, and stacking.
    /// Submitting the command recomputes the route against the then-current state.
    ///
    /// # Parameters
    ///
    /// - `unit_id`: Identifier of an existing unit controlled by the acting side.
    /// - `mode`: Movement system to preview: Tactical, March, Rail, or Air Transport.
    ///
    /// # Returns
    ///
    /// Legal destinations sorted by hex identifier, with each route and its cost.
    /// An empty list means that the usable system currently has no legal destination.
    ///
    /// # Errors
    ///
    /// Returns a [`RuleError`] for an invalid phase or ownership, or when the unit
    /// cannot use the requested system because of its state, supply, remaining
    /// allowance, transport capacity, or other movement prerequisites.
    pub fn movement_options(
        &self,
        unit_id: &UnitId,
        mode: MovementMode,
    ) -> Result<Vec<MovementOption>, RuleError> {
        let (side_id, phase) = self.movement_phase()?;
        let unit = self.owned_unit(unit_id, &side_id)?;
        let context = MovementContext::new(self, &side_id);
        self.movement_prerequisites(&context, unit, mode, phase)?;

        let mut options = Vec::new();
        match mode {
            MovementMode::Tactical | MovementMode::March => {
                let ground = self.ground_search(&context, unit, mode, phase)?;
                for hex_id in ground.search.cost.keys() {
                    if *hex_id == ground.search.origin {
                        continue;
                    }
                    let Ok((path, cost)) = ground.route(hex_id) else {
                        continue;
                    };
                    let destination = HexId(hex_id.clone());
                    if self
                        .check_destination(&context, unit, &destination, mode)
                        .is_ok()
                    {
                        options.push(MovementOption {
                            hex_id: destination,
                            cost,
                            path,
                        });
                    }
                }
                if mode == MovementMode::Tactical {
                    options.extend(self.danish_ferry_options(&context, unit, phase)?);
                }
            }
            MovementMode::Rail => {
                let (search, remaining) = self.rail_search(&context, unit)?;
                for (hex_id, &cost) in &search.cost {
                    let destination = HexId(hex_id.clone());
                    if *hex_id == search.origin
                        || cost > remaining
                        || self
                            .check_destination(&context, unit, &destination, mode)
                            .is_err()
                    {
                        continue;
                    }
                    options.push(MovementOption {
                        path: search.path_to(hex_id),
                        hex_id: destination,
                        cost,
                    });
                }
            }
            MovementMode::AirTransport | MovementMode::Paradrop | MovementMode::SeaTransport => {
                let search = self.lift_search(&context, unit, mode);
                for hex in self.scenario.map.hexes.iter() {
                    if matches!(&unit.location, UnitLocation::Hex { hex_id } if *hex_id == hex.id)
                        || self
                            .check_destination(&context, unit, &hex.id, mode)
                            .is_err()
                    {
                        continue;
                    }
                    if let Some(path) = search.path(&hex.id.0) {
                        options.push(MovementOption {
                            hex_id: hex.id.clone(),
                            cost: 0,
                            path,
                        });
                    }
                }
            }
        }
        options.sort_by(|a, b| a.hex_id.0.cmp(&b.hex_id.0));
        Ok(options)
    }

    /// Validates one movement order and returns its route and cost.
    pub(crate) fn plan_movement(
        &self,
        side_id: &SideId,
        unit: &UnitState,
        destination: &HexId,
        mode: MovementMode,
        phase: MovementPhase,
    ) -> Result<(Vec<HexId>, u16), RuleError> {
        let context = MovementContext::new(self, side_id);
        if !context.hexes.contains_key(destination.0.as_str()) {
            return Err(RuleError::new(
                "unknownHex",
                format!("Unknown destination: {}", destination.0),
            ));
        }
        if matches!(&unit.location, UnitLocation::Hex { hex_id } if hex_id == destination) {
            return Err(RuleError::new(
                "sameDestination",
                "The unit already occupies that hex",
            ));
        }
        self.movement_prerequisites(&context, unit, mode, phase)?;
        self.check_destination(&context, unit, destination, mode)?;

        match mode {
            MovementMode::Tactical | MovementMode::March => {
                if mode == MovementMode::Tactical {
                    if let Some(option) = self
                        .danish_ferry_options(&context, unit, phase)?
                        .into_iter()
                        .find(|option| option.hex_id == *destination)
                    {
                        return Ok((option.path, option.cost));
                    }
                }
                self.ground_search(&context, unit, mode, phase)?
                    .route(&destination.0)
            }
            MovementMode::Rail => {
                let (search, remaining) = self.rail_search(&context, unit)?;
                let cost = search.cost.get(&destination.0).copied().ok_or_else(|| {
                    RuleError::new(
                        "noLegalRoute",
                        "No legal rail route reaches that destination",
                    )
                })?;
                if cost > remaining {
                    return Err(RuleError::new(
                        "railMovementLimitExceeded",
                        format!("Rail route is {cost} hexes; {remaining} remain"),
                    ));
                }
                Ok((search.path_to(&destination.0), cost))
            }
            MovementMode::AirTransport | MovementMode::Paradrop | MovementMode::SeaTransport => {
                let path = self
                    .lift_search(&context, unit, mode)
                    .path(&destination.0)
                    .ok_or_else(|| {
                        RuleError::new(
                            "noLegalRoute",
                            match mode {
                                MovementMode::SeaTransport => {
                                    "No sea route outside enemy Airspace reaches that port"
                                }
                                _ => "No flight path outside enemy Airspace reaches that city",
                            },
                        )
                    })?;
                Ok((path, 0))
            }
        }
    }

    /// Returns whether a hex is in an enemy zone of control for the given side.
    pub(crate) fn hex_in_enemy_zoc(&self, side_id: &SideId, hex_id: &HexId) -> bool {
        MovementContext::new(self, side_id).in_enemy_zoc(&hex_id.0)
    }

    /// Checks that do not depend on the destination.
    fn movement_prerequisites(
        &self,
        context: &MovementContext,
        unit: &UnitState,
        mode: MovementMode,
        phase: MovementPhase,
    ) -> Result<(), RuleError> {
        let movements = self.phase_movements(phase)?;
        if phase == MovementPhase::Reserve {
            if !self.reserve_state()?.unit_ids.contains(unit.id()) {
                return Err(RuleError::new(
                    "notInReserve",
                    "Only units under a Reserve/OMG Marker move in the Reserve Phase",
                ));
            }
            // 12.2.2, 28.2.1: Tactical (or Minimum) movement only.
            if mode != MovementMode::Tactical {
                return Err(RuleError::new(
                    "reserveTacticalOnly",
                    "Reserve movement uses Tactical or Minimum movement only",
                ));
            }
        }
        if has_incompatible_movement(movements, unit.id(), mode) {
            return Err(RuleError::new(
                "movementModeAlreadyUsed",
                "A unit may use only one movement system during a planning phase",
            ));
        }
        let origin = match &unit.location {
            UnitLocation::Hex { hex_id } => Some(hex_id.0.as_str()),
            UnitLocation::StrategicReserve => None,
        };
        let origin_zoc = origin.is_some_and(|id| context.in_enemy_zoc(id));
        if unit.has_trait("immobile") {
            return Err(RuleError::new(
                "unitImmobile",
                "The scenario does not allow this unit to move",
            ));
        }
        // 25.6.4 (1): a Disrupted or Suppressed unit may use only Minimum movement.
        if unit.disruption.is_some() && mode != MovementMode::Tactical {
            return Err(RuleError::new(
                "unitDisrupted",
                "A Disrupted or Suppressed unit may move only one hex by Minimum movement",
            ));
        }
        match mode {
            MovementMode::Tactical | MovementMode::March => {
                if origin.is_none() {
                    return Err(RuleError::new(
                        "unitOffMap",
                        "Ground movement requires a unit already on the map",
                    ));
                }
                if unit.train_status.is_some() {
                    return Err(RuleError::new(
                        "unitUnderTrainMarker",
                        "Detrain the unit before using ground movement",
                    ));
                }
                // 12.3.2: a Soft unit that has entered an EZOC must stop there.
                let already_moved = movements.iter().any(|m| m.unit_id == *unit.id());
                if already_moved && origin_zoc && !unit.is_hard() {
                    return Err(RuleError::new(
                        "mustStopInEnemyZoc",
                        "Only Hard units may continue moving after entering an enemy zone of control",
                    ));
                }
                if mode == MovementMode::March {
                    // 12.4.2, 12.4 (1), 12.4 (3).
                    if unit.is_headquarters() {
                        return Err(RuleError::new(
                            "marchUnavailable",
                            "Headquarters may not use march movement",
                        ));
                    }
                    if unit.supply.movement == Some(SupplyStatus::OutOfSupply) {
                        return Err(RuleError::new(
                            "marchUnavailable",
                            "Out-of-movement-supply units may not use march movement",
                        ));
                    }
                    if origin_zoc {
                        return Err(RuleError::new(
                            "marchUnavailable",
                            "March movement may not start in an enemy zone of control",
                        ));
                    }
                    // 12.4, 11.8.1: March starts and stays in friendly Airspace.
                    if origin.is_some_and(|id| !context.friendly_airspace(id)) {
                        return Err(RuleError::new(
                            "marchUnavailable",
                            "March movement must start in friendly Airspace",
                        ));
                    }
                }
            }
            MovementMode::Rail => {
                if unit.train_status != Some(TrainStatus::Entrained) {
                    return Err(RuleError::new(
                        "unitNotEntrained",
                        "A unit must finish entraining before it can move by rail",
                    ));
                }
                if origin.is_none() {
                    return Err(RuleError::new(
                        "railEntryNotImplemented",
                        "Strategic Reserve rail entry requires a Reinforcement Box and cannot yet enter the map directly",
                    ));
                }
                // 13.2 (5): rail movement may not leave contested or enemy Airspace.
                if origin.is_some_and(|id| !context.friendly_airspace(id)) {
                    return Err(RuleError::new(
                        "notFriendlyAirspace",
                        "Rail movement must start in friendly Airspace",
                    ));
                }
                // 13.2 (2): rail movement may not leave an EZOC.
                if origin_zoc {
                    return Err(RuleError::new(
                        "enemyZoneOfControl",
                        "Rail movement may not leave an enemy zone of control",
                    ));
                }
            }
            MovementMode::AirTransport | MovementMode::Paradrop | MovementMode::SeaTransport => {
                self.lift_prerequisites(context, unit, mode, origin, movements)?;
            }
        }
        Ok(())
    }

    /// Checks that depend on where the unit would end its movement.
    fn check_destination(
        &self,
        context: &MovementContext,
        unit: &UnitState,
        destination: &HexId,
        mode: MovementMode,
    ) -> Result<(), RuleError> {
        let hex = context.hexes.get(destination.0.as_str()).ok_or_else(|| {
            RuleError::new(
                "unknownHex",
                format!("Unknown destination: {}", destination.0),
            )
        })?;
        if hex.terrain == Terrain::Sea {
            return Err(RuleError::new(
                "prohibitedTerrain",
                "Ground units cannot end movement in an all-sea hex",
            ));
        }
        if context.enemy_free_cities.contains(destination.0.as_str()) {
            return Err(RuleError::new(
                "enemyFreeCity",
                "An enemy Free City can be entered only by advancing after a battle",
            ));
        }
        match mode {
            MovementMode::AirTransport | MovementMode::SeaTransport => {
                let sea = mode == MovementMode::SeaTransport;
                // House rule: air transport flies city to city, sea transport sails
                // port to port; either way to a city the side controls.
                if hex.city.is_none() || (sea && hex.port.is_none()) {
                    return Err(RuleError::new(
                        "invalidLiftDestination",
                        if sea {
                            "Sea transport must end in a port"
                        } else {
                            "Air transport must end in a city"
                        },
                    ));
                }
                if self.city_control.get(destination) != Some(&unit.definition.side_id) {
                    return Err(RuleError::new(
                        "enemyControlledCity",
                        "Transport must end in a city the side controls",
                    ));
                }
                // 16.1.2, 16.2.2: the route may not enter enemy Airspace.
                if context.airspace.of(&destination.0) == Airspace::Enemy {
                    return Err(RuleError::new(
                        "enemyAirspace",
                        "Transport may not end in enemy Airspace",
                    ));
                }
                // 16.1.1 (5c): not in Mountain terrain.
                if !sea && hex.terrain == Terrain::Mountain {
                    return Err(RuleError::new(
                        "invalidLiftDestination",
                        "Air transport may not end in mountain terrain",
                    ));
                }
                // 16.1.1 (5a), 16.2.1 (5a): not in an EZOC; friendly units do not negate it.
                if context.in_enemy_zoc(&destination.0) {
                    return Err(RuleError::new(
                        "enemyZoneOfControl",
                        "Transport may not end in an enemy zone of control",
                    ));
                }
            }
            MovementMode::Paradrop => {
                // 16.1.3 (2): Clear or Marsh only; enemy Airspace and EZOCs are allowed.
                if !matches!(hex.terrain, Terrain::Clear | Terrain::Marsh) {
                    return Err(RuleError::new(
                        "invalidLiftDestination",
                        "A Paradrop must land in Clear or Marsh terrain",
                    ));
                }
                // 30.1.1: air movement never ends in an enemy-controlled city.
                if context
                    .enemy_conquered_cities
                    .contains(destination.0.as_str())
                {
                    return Err(RuleError::new(
                        "enemyControlledCity",
                        "A Paradrop may not land in an enemy-controlled city",
                    ));
                }
            }
            _ => {}
        }
        self.validate_destination_stacking(unit, destination)
    }

    /// Ground search for Tactical (12.2, 12.3) or March (12.4) movement.
    fn ground_search(
        &self,
        context: &MovementContext,
        unit: &UnitState,
        mode: MovementMode,
        phase: MovementPhase,
    ) -> Result<GroundSearch, RuleError> {
        let UnitLocation::Hex { hex_id: origin } = &unit.location else {
            return Err(RuleError::new("unitOffMap", "Unit is not on the map"));
        };
        let printed = unit
            .current_step()
            .ok_or_else(|| RuleError::new("invalidUnit", "Unit has no active strength step"))?
            .movement;
        let allowance = match mode {
            // 25.6.4 (1): Disrupted units have only Minimum movement.
            _ if unit.disruption.is_some() => 0,
            // 28.2.1: half the printed allowance, rounding fractions up.
            _ if phase == MovementPhase::Reserve => printed.div_ceil(2),
            MovementMode::March => printed.saturating_mul(2),
            // 10.4.3 (1), 10.5: unsupplied units and HQs move at half allowance.
            _ if unit.supply.movement == Some(SupplyStatus::OutOfSupply)
                || unit.supply.headquarters == Some(SupplyStatus::OutOfSupply) =>
            {
                printed / 2
            }
            _ => printed,
        };
        let movements = self.phase_movements(phase)?;
        let spent = movement_spent(movements, unit.id(), mode);
        let first_move = !movements.iter().any(|m| m.unit_id == *unit.id());
        let rules = &self.scenario.battle_planning_rules;
        let march = mode == MovementMode::March;
        let hard = unit.is_hard();
        let origin_id = origin.0.as_str();
        // 25.9.3, 28.2.1: in the Reserve Phase, Hard units using Tactical
        // movement ignore the +1 to enter or leave an EZOC hex in a Breakthrough Zone.
        let breakthrough =
            phase == MovementPhase::Reserve && hard && mode == MovementMode::Tactical;
        let zoc_cost = |id: &str| {
            u16::from(
                context.in_enemy_zoc(id)
                    && !(breakthrough && context.breakthrough_zone.contains(id)),
            )
        };

        let remaining = allowance.saturating_sub(spent);
        let step_cost = |current: &str, next: &str| {
            let current_zoc = context.in_enemy_zoc(current);
            // 12.3 (4), 12.3.2: Soft units stop in the first EZOC hex they enter.
            if current_zoc && current != origin_id && !hard {
                return None;
            }
            let mut cost = rules.hex_entry_cost(context.hexes.get(next)?)?;
            // 12.1.2, 30.2.1: never into enemy units or an enemy Free City.
            if context.prohibited_hexside(current, next)
                || context.enemy_occupied.contains(next)
                || context.enemy_free_cities.contains(next)
            {
                return None;
            }
            let next_zoc = context.in_enemy_zoc(next);
            let interdicted = context.enemy_interdiction.contains(next);
            // 8.1.2 (3): March movement may never enter an EZOC.
            // 30.3.1: only Tactical movement may enter an enemy Conquered City.
            // 11.8.1: March stays in friendly Airspace. 23.8: nor may it enter an
            // enemy Air Interdiction Zone.
            if march
                && (next_zoc
                    || context.enemy_conquered_cities.contains(next)
                    || !context.friendly_airspace(next)
                    || interdicted)
            {
                return None;
            }
            // 12.3.2: Soft units move EZOC to EZOC only into a hex holding a
            // friendly unit or friendly Free City.
            if !hard && current_zoc && next_zoc && !context.negates_ezoc_for_entry(next) {
                return None;
            }
            let features = context.side_features(current, next);
            if features.contains(&HexsideFeature::MajorRiver) {
                cost += rules.major_river_cost;
            } else if features.contains(&HexsideFeature::MinorRiver) {
                cost += rules.minor_river_cost;
            }
            // 12.3 (1), (2): +1 to enter and +1 to leave an EZOC hex.
            // 23.8: +1 to enter an enemy Air Interdiction Zone hex.
            Some(cost + zoc_cost(current) + zoc_cost(next) + u16::from(interdicted))
        };
        let neighbors = |id: &str| -> Vec<String> {
            context
                .neighbors(id)
                .into_iter()
                .map(str::to_owned)
                .collect()
        };
        // Nothing beyond the remaining allowance can be a legal destination.
        let search = dijkstra(
            origin_id,
            &|current, next, cost| {
                step_cost(current, next).filter(|step| cost.saturating_add(*step) <= remaining)
            },
            &neighbors,
        );
        let minimum_moves = if first_move {
            context
                .neighbors(origin_id)
                .into_iter()
                .filter_map(|next| step_cost(origin_id, next).map(|cost| (next.to_owned(), cost)))
                .collect()
        } else {
            HashMap::new()
        };
        Ok(GroundSearch {
            search,
            remaining,
            minimum_moves,
        })
    }

    /// Finds rail routes within the remaining rail allowance while excluding enemy ZOCs and prohibited terrain (13.2).
    fn rail_search(
        &self,
        context: &MovementContext,
        unit: &UnitState,
    ) -> Result<(Search, u16), RuleError> {
        // Rail search (13.2): up to the rail limit through friendly land hexes,
        // never entering an EZOC or crossing Prohibited Terrain.
        let UnitLocation::Hex { hex_id: origin } = &unit.location else {
            return Err(RuleError::new("unitOffMap", "Unit is not on the map"));
        };
        let spent = movement_spent(
            &self.active_plan()?.movements,
            unit.id(),
            MovementMode::Rail,
        );
        let remaining = self
            .scenario
            .battle_planning_rules
            .rail_movement_limit
            .saturating_sub(spent);
        let edge = |current: &str, next: &str, cost: u16| {
            let hex = context.hexes.get(next)?;
            if cost >= remaining
                || hex.terrain == Terrain::Sea
                || context.prohibited_hexside(current, next)
                || context.enemy_occupied.contains(next)
                || context.enemy_free_cities.contains(next)
                || context.enemy_conquered_cities.contains(next)
                || context.in_enemy_zoc(next)
                // 13.2 (3), (5): only friendly Airspace, never an Air Interdiction Zone.
                || !context.friendly_airspace(next)
                || context.enemy_interdiction.contains(next)
            {
                return None;
            }
            Some(1)
        };
        let neighbors = |id: &str| -> Vec<String> {
            context
                .neighbors(id)
                .into_iter()
                .map(str::to_owned)
                .collect()
        };
        Ok((dijkstra(&origin.0, &edge, &neighbors), remaining))
    }

    /// Lists legal NATO Danish Ferry crossings available as Minimum movement in this segment (12.8).
    fn danish_ferry_options(
        &self,
        context: &MovementContext,
        unit: &UnitState,
        phase: MovementPhase,
    ) -> Result<Vec<MovementOption>, RuleError> {
        // 12.8: one NATO unit per direction per movement segment may cross the
        // Danish Ferry hexside as Minimum movement.  The Warsaw Pact may use it only
        // after Denmark surrenders, which is not yet implemented.
        let UnitLocation::Hex { hex_id: origin } = &unit.location else {
            return Ok(Vec::new());
        };
        let movements = self.phase_movements(phase)?;
        if unit.definition.side_id.0 != "nato" || movements.iter().any(|m| m.unit_id == *unit.id())
        {
            return Ok(Vec::new());
        }
        let mut options = Vec::new();
        for next in context.neighbors(&origin.0) {
            if !context
                .side_features(&origin.0, next)
                .contains(&HexsideFeature::DanishFerry)
            {
                continue;
            }
            let used = movements.iter().any(|m| {
                m.to.0 == next
                    && matches!(&m.from, UnitLocation::Hex { hex_id } if *hex_id == *origin)
            });
            // 12.5 (2): Soft units cannot move EZOC to EZOC unless the hex is friendly-occupied.
            let zoc_blocked = !unit.is_hard()
                && context.in_enemy_zoc(&origin.0)
                && context.in_enemy_zoc(next)
                && !context.negates_ezoc_for_entry(next);
            let destination = HexId(next.to_owned());
            if used
                || zoc_blocked
                || context.enemy_occupied.contains(next)
                || self
                    .check_destination(context, unit, &destination, MovementMode::Tactical)
                    .is_err()
            {
                continue;
            }
            // Minimum movement ends the unit's movement: spend the whole allowance.
            let allowance = unit.current_step().map_or(1, |step| step.movement).max(1);
            options.push(MovementOption {
                hex_id: destination.clone(),
                cost: allowance,
                path: vec![destination],
            });
        }
        Ok(options)
    }

    /// Rejects enemy occupants, excess maneuver steps, or a second friendly HQ at the destination.
    /// 16.1.1, 16.2.1, and the house rules for lift orders: who may fly, drop,
    /// or sail, where the order may start, and the side's Lift Command capacity.
    fn lift_prerequisites(
        &self,
        context: &MovementContext,
        unit: &UnitState,
        mode: MovementMode,
        origin: Option<&str>,
        movements: &[PlannedMovement],
    ) -> Result<(), RuleError> {
        let (name, eligible, reason) = match mode {
            MovementMode::AirTransport => (
                "Air transport",
                unit.is_air_transportable(),
                "Only Airborne and Airmobile units may use air transport",
            ),
            // 16.1.3 (1): only units with the Airborne symbol may Paradrop.
            MovementMode::Paradrop => (
                "A Paradrop",
                unit.has_trait("airborne"),
                "Only Airborne units may Paradrop",
            ),
            _ => ("Sea transport", true, ""),
        };
        if !eligible {
            return Err(RuleError::new("unitNotAirTransportable", reason));
        }
        if unit.train_status.is_some() {
            return Err(RuleError::new(
                "unitUnderTrainMarker",
                format!(
                    "A unit under a train marker may not use {}",
                    name.to_lowercase()
                ),
            ));
        }
        if unit.supply.movement == Some(SupplyStatus::OutOfSupply)
            || unit.supply.headquarters == Some(SupplyStatus::OutOfSupply)
        {
            return Err(RuleError::new(
                "unitOutOfSupply",
                format!("An out-of-supply unit may not use {}", name.to_lowercase()),
            ));
        }
        if movements.iter().any(|m| m.unit_id == *unit.id()) {
            return Err(RuleError::new(
                "unitAlreadyMoved",
                "A unit that has moved may not use another movement system",
            ));
        }
        if let Some(origin) = origin {
            let hex = context.hexes[origin];
            let sea = mode == MovementMode::SeaTransport;
            if hex.city.is_none() || (sea && hex.port.is_none()) {
                return Err(RuleError::new(
                    "invalidLiftOrigin",
                    if sea {
                        "Sea transport must start in a port or the Strategic Reserve"
                    } else {
                        "Air movement must start in a city or the Strategic Reserve"
                    },
                ));
            }
            // 16.1.1 (2), 16.2.1 (2): friendly or contested Airspace.
            if context.airspace.of(origin) == Airspace::Enemy {
                return Err(RuleError::new(
                    "enemyAirspace",
                    format!("{name} may not start in enemy Airspace"),
                ));
            }
            // 16.1.1 (5a), 16.2.1 (5a): never from an EZOC; friendly units do not negate it.
            if context.in_enemy_zoc(origin) {
                return Err(RuleError::new(
                    "enemyZoneOfControl",
                    format!("{name} may not start in an enemy zone of control"),
                ));
            }
            // 16.1.1 (5c).
            if !sea && hex.terrain == Terrain::Mountain {
                return Err(RuleError::new(
                    "invalidLiftOrigin",
                    "Air movement may not start in mountain terrain",
                ));
            }
        }
        // 3.8, 16.0: each Lift Command carries one step per turn.
        let rules = &self.scenario.battle_planning_rules;
        let side_id = &unit.definition.side_id;
        let plan = self.active_plan()?;
        let (capacity, used, kind, code) = if mode == MovementMode::SeaTransport {
            (
                rules.sealift_commands(side_id),
                plan.sealift_steps_used,
                "Sealift",
                "sealiftCapacityExceeded",
            )
        } else {
            (
                rules.airlift_commands(side_id),
                plan.airlift_steps_used,
                "Airlift",
                "airliftCapacityExceeded",
            )
        };
        if used + unit.step_count() > capacity {
            return Err(RuleError::new(
                code,
                format!("Moving this unit needs more than the side's {capacity} {kind} Commands"),
            ));
        }
        Ok(())
    }

    /// Where a lift order can reach, with a route to each hex.
    ///
    /// Air transport flies over any hex outside enemy Airspace, crossing EZOCs
    /// freely (house rule). Sea transport follows All-Sea, Coastal, and
    /// major-river hexes outside enemy Airspace, a river hex only clear of
    /// EZOCs (16.2.1.1). A Paradrop's flight is not restricted. Units leaving
    /// the Strategic Reserve start from the side's sector entry hexes (air) or
    /// its map-edge sea hexes (sea).
    fn lift_search(
        &self,
        context: &MovementContext,
        unit: &UnitState,
        mode: MovementMode,
    ) -> LiftSearch {
        let side_id = &unit.definition.side_id;
        let open_sky = |id: &str| context.airspace.of(id) != Airspace::Enemy;
        let river = |id: &str| {
            context.neighbors(id).into_iter().any(|next| {
                context
                    .side_features(id, next)
                    .contains(&HexsideFeature::MajorRiver)
            })
        };
        let at_sea = |id: &str| {
            let hex = context.hexes[id];
            hex.terrain == Terrain::Sea
                || hex.coastal == Some(true)
                || hex.port.is_some()
                || (river(id) && !context.in_enemy_zoc(id))
        };
        let passable = |id: &str| match mode {
            MovementMode::Paradrop => true,
            MovementMode::SeaTransport => open_sky(id) && at_sea(id),
            _ => open_sky(id),
        };
        let starts: Vec<String> = match &unit.location {
            UnitLocation::Hex { hex_id } => vec![hex_id.0.clone()],
            UnitLocation::StrategicReserve if mode == MovementMode::SeaTransport => {
                let west = self
                    .scenario
                    .map
                    .reinforcement_sectors
                    .iter()
                    .filter(|sector| sector.side_id == *side_id)
                    .any(|sector| sector.hex_id.0.ends_with("34"));
                context
                    .hexes
                    .values()
                    .filter(|hex| {
                        let outward = if west {
                            hex.col + 1
                        } else {
                            hex.col.saturating_sub(1)
                        };
                        let edge = !context
                            .hexes
                            .contains_key(format!("{:02}{:02}", hex.row, outward).as_str());
                        edge && hex.terrain == Terrain::Sea && passable(&hex.id.0)
                    })
                    .map(|hex| hex.id.0.clone())
                    .collect()
            }
            UnitLocation::StrategicReserve => self
                .scenario
                .map
                .reinforcement_sectors
                .iter()
                .filter(|sector| sector.side_id == *side_id && passable(&sector.hex_id.0))
                .map(|sector| sector.hex_id.0.clone())
                .collect(),
        };
        let from_map = matches!(unit.location, UnitLocation::Hex { .. });
        let mut previous: HashMap<String, Option<String>> = HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        for start in starts {
            previous.insert(start.clone(), None);
            queue.push_back(start);
        }
        while let Some(current) = queue.pop_front() {
            for next in context.neighbors(&current) {
                // Flights and voyages ignore hexside barriers.
                if previous.contains_key(next) || !passable(next) {
                    continue;
                }
                previous.insert(next.to_owned(), Some(current.clone()));
                queue.push_back(next.to_owned());
            }
        }
        LiftSearch { previous, from_map }
    }

    pub(crate) fn validate_destination_stacking(
        &self,
        moving: &UnitState,
        destination: &HexId,
    ) -> Result<(), RuleError> {
        let occupants: Vec<_> = self
            .units
            .values()
            .filter(|unit| {
                unit.id() != moving.id()
                    && matches!(&unit.location, UnitLocation::Hex { hex_id } if hex_id == destination)
            })
            .collect();
        // 12.1.2: ground movement may never enter an enemy-occupied hex.
        if occupants
            .iter()
            .any(|unit| unit.definition.side_id != moving.definition.side_id)
        {
            return Err(RuleError::new(
                "enemyOccupiedDestination",
                "Movement may not end in an enemy-occupied hex",
            ));
        }
        // 9.1.1: four Maneuver steps plus one HQ.
        if moving.is_headquarters() {
            if occupants.iter().any(|unit| unit.is_headquarters()) {
                return Err(RuleError::new(
                    "stackingLimitExceeded",
                    "Only one headquarters may occupy a hex",
                ));
            }
        } else {
            let steps: u16 = occupants
                .iter()
                .filter(|unit| !unit.is_headquarters())
                .map(|unit| unit.step_count())
                .sum();
            let limit = self.scenario.battle_planning_rules.maneuver_stacking_limit;
            if steps + moving.step_count() > limit {
                return Err(RuleError::new(
                    "stackingLimitExceeded",
                    format!("The destination would exceed the {limit}-step stacking limit"),
                ));
            }
        }
        Ok(())
    }
}
