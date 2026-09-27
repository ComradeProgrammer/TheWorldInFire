//! Offensive Strike Phase (rules 20 and 23): Air Strikes and Air Interdiction.
//!
//! Nuclear, Chemical, and Artillery Strikes and NATO Deep Interdiction are not
//! implemented; the phase consists of the Air Strike Segment.

use crate::error::RuleError;
use crate::event::GameEvent;
use crate::model::{
    AirInterdictionZone, AirMission, AirMissionKind, AirPointKind, AirPointSource,
    AirStrikeOptions, Airspace, CityKind, Disruption, HexId, MapHex, PhaseActor, SideId,
    StrikePlan, StrikeResolution, StrikeResult, StrikeTargetHex, StrikeTargetUnit, Terrain, UnitId,
    UnitLocation, UnitState,
};
use crate::state::GameState;

/// Steps one Air Point may strike (23.1.5).
const STEPS_PER_AIR_STRIKE: u16 = 2;
/// Air Strikes one hex may receive per segment (23.1.5).
const AIR_STRIKES_PER_HEX: usize = 2;

/// Strike Table, one Air or Artillery Point column.
pub(crate) fn strike_table(modified_roll: i8) -> StrikeResult {
    match modified_roll {
        i8::MIN..=1 => StrikeResult::NoEffect,
        2..=4 => StrikeResult::Disrupted,
        _ => StrikeResult::StepLoss,
    }
}

fn is_tactical_airspace(airspace: Airspace) -> bool {
    matches!(airspace, Airspace::Friendly | Airspace::Contested)
}

impl GameState {
    // ------------------------------------------------------------------ phase hooks

    /// 23.1.1, 23.1.7: fresh Air Points each Joint Reinforcement Phase; unused
    /// points are lost. One-time bonus points carry over until spent.
    pub(crate) fn reset_air_points(&mut self, events: &mut Vec<GameEvent>) {
        let rules = &self.scenario.battle_planning_rules.air_power;
        for points in &mut self.air_points {
            let power = rules.for_side(&points.side_id);
            points.tactical = power.tactical_per_turn;
            points.operational = power.operational_per_turn;
        }
        events.push(GameEvent::AirPointsReset {
            air_points: self.air_points.clone(),
        });
    }

    pub(crate) fn start_strike_plan(&mut self) {
        let Some(PhaseActor::Side { side_id }) = self.current_step().map(|step| step.actor.clone())
        else {
            return;
        };
        self.strike_plan = Some(StrikePlan {
            game_turn: self.game_turn,
            side_id,
            missions: Vec::new(),
            resolved: false,
            next_mission_id: 1,
        });
    }

    /// Ending the phase resolves any committed missions that were not yet rolled.
    pub(crate) fn finish_offensive_strike(&mut self, events: &mut Vec<GameEvent>) {
        if self.strike_plan.as_ref().is_some_and(|plan| !plan.resolved) {
            events.extend(self.resolve_missions());
        }
        self.strike_plan = None;
    }

    /// 23.8.2, 28.2.5: at the end of a side's Reserve Phase, enemy Air
    /// Interdiction Zones and the side's Breakthrough Markers are removed.
    pub(crate) fn finish_reserve(&mut self, events: &mut Vec<GameEvent>) {
        let Some(PhaseActor::Side { side_id }) = self.current_step().map(|step| step.actor.clone())
        else {
            return;
        };
        let (removed, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.air_interdiction_zones)
            .into_iter()
            .partition(|zone| zone.side_id != side_id);
        self.air_interdiction_zones = kept;
        let mut by_side: Vec<(SideId, Vec<HexId>)> = Vec::new();
        for zone in removed {
            match by_side.iter_mut().find(|(side, _)| *side == zone.side_id) {
                Some((_, hexes)) => hexes.push(zone.hex_id),
                None => by_side.push((zone.side_id, vec![zone.hex_id])),
            }
        }
        for (side_id, hex_ids) in by_side {
            events.push(GameEvent::AirInterdictionZonesRemoved { side_id, hex_ids });
        }
        if !self.breakthrough_markers.is_empty() {
            events.push(GameEvent::BreakthroughMarkersRemoved {
                hex_ids: std::mem::take(&mut self.breakthrough_markers),
            });
        }
    }

    /// Recovery (7.2 C): the acting side's Disrupted markers come off once its
    /// movement is complete, i.e. when its Battle Planning Phase ends.
    pub(crate) fn remove_disruption(&mut self, side_id: &SideId, events: &mut Vec<GameEvent>) {
        self.clear_marker(side_id, Disruption::Disrupted, events);
    }

    /// Unsuppression Phase (7.2 H): the acting side's Suppressed markers come off.
    pub(crate) fn remove_suppression(&mut self, events: &mut Vec<GameEvent>) {
        let Some(PhaseActor::Side { side_id }) = self.current_step().map(|step| step.actor.clone())
        else {
            return;
        };
        self.clear_marker(&side_id, Disruption::Suppressed, events);
    }

    fn clear_marker(&mut self, side_id: &SideId, marker: Disruption, events: &mut Vec<GameEvent>) {
        for unit in self
            .units
            .values_mut()
            .filter(|unit| unit.definition.side_id == *side_id && unit.disruption == Some(marker))
        {
            unit.disruption = None;
            events.push(GameEvent::UnitDisruptionChanged {
                unit_id: unit.id().clone(),
                disruption: None,
            });
        }
    }

    // ------------------------------------------------------------------ commands

    pub(super) fn plan_air_strike(
        &mut self,
        hex_id: HexId,
        unit_ids: Vec<UnitId>,
        air_point: AirPointKind,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.offensive_strike_side()?;
        self.open_strike_plan()?;
        let hex = self.map_hex(&hex_id)?;
        if unit_ids.is_empty() {
            return Err(RuleError::new(
                "invalidStrikeTarget",
                "An Air Strike must name at least one enemy unit",
            ));
        }
        let mut steps = 0;
        let mut targets = Vec::new();
        for unit_id in &unit_ids {
            if targets.iter().any(|unit: &&UnitState| unit.id() == unit_id) {
                return Err(RuleError::new(
                    "invalidStrikeTarget",
                    "A unit may be named only once in an Air Strike",
                ));
            }
            let unit = self.units.get(unit_id).ok_or_else(|| {
                RuleError::new("unknownUnit", format!("Unknown unit: {}", unit_id.0))
            })?;
            if unit.definition.side_id == side_id
                || !matches!(&unit.location, UnitLocation::Hex { hex_id: at } if *at == hex_id)
            {
                return Err(RuleError::new(
                    "invalidStrikeTarget",
                    "Air Strike targets must be enemy units in the target hex",
                ));
            }
            steps += unit.step_count();
            targets.push(unit);
        }
        // 23.1.5: up to two steps — two one-step units or one two-step unit.
        if steps > STEPS_PER_AIR_STRIKE {
            return Err(RuleError::new(
                "invalidStrikeTarget",
                "One Air Point strikes at most two steps",
            ));
        }
        // 23.5: an HQ is struck alone and only with Operational Air Points.
        if targets.iter().any(|unit| unit.is_headquarters()) {
            if targets.len() > 1 {
                return Err(RuleError::new(
                    "invalidStrikeTarget",
                    "An Air Strike on an HQ may not target any other unit",
                ));
            }
            if air_point != AirPointKind::Operational {
                return Err(RuleError::new(
                    "operationalPointRequired",
                    "Only Operational Air Points may strike an HQ",
                ));
            }
        }
        let plan = self.strike_plan.as_ref().expect("checked open");
        let strikes_here: Vec<&AirMission> = plan
            .missions
            .iter()
            .filter(|mission| {
                mission.hex_id == hex_id && matches!(mission.kind, AirMissionKind::Strike { .. })
            })
            .collect();
        // 23.1.5, 20.1: no unit is struck twice in the segment; at most two Air Points per hex.
        if strikes_here.iter().any(|mission| match &mission.kind {
            AirMissionKind::Strike { unit_ids: struck } => {
                struck.iter().any(|id| unit_ids.contains(id))
            }
            AirMissionKind::Interdiction => false,
        }) {
            return Err(RuleError::new(
                "unitAlreadyTargeted",
                "A unit may be the target of only one Air Strike per segment",
            ));
        }
        if strikes_here.len() >= AIR_STRIKES_PER_HEX {
            return Err(RuleError::new(
                "hexStrikeLimit",
                "A hex may receive at most two Air Strikes per segment",
            ));
        }
        let airspace = self.airspace_map(&side_id).of(&hex.id.0);
        let source = self.take_air_point(&side_id, air_point, airspace)?;
        self.push_mission(side_id, hex_id, AirMissionKind::Strike { unit_ids }, source)
    }

    pub(super) fn plan_air_interdiction(
        &mut self,
        hex_id: HexId,
        air_point: AirPointKind,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.offensive_strike_side()?;
        self.open_strike_plan()?;
        let hex = self.map_hex(&hex_id)?;
        if hex.terrain == Terrain::Sea {
            return Err(RuleError::new(
                "invalidInterdictionHex",
                "An Air Interdiction Marker must be placed on a land hex",
            ));
        }
        let duplicate = self
            .air_interdiction_zones
            .iter()
            .any(|zone| zone.side_id == side_id && zone.hex_id == hex_id)
            || self.strike_plan.as_ref().is_some_and(|plan| {
                plan.missions.iter().any(|mission| {
                    mission.hex_id == hex_id && mission.kind == AirMissionKind::Interdiction
                })
            });
        if duplicate {
            return Err(RuleError::new(
                "invalidInterdictionHex",
                "The hex already holds a friendly Air Interdiction Marker",
            ));
        }
        let airspace = self.airspace_map(&side_id).of(&hex.id.0);
        let source = self.take_air_point(&side_id, air_point, airspace)?;
        self.push_mission(side_id, hex_id, AirMissionKind::Interdiction, source)
    }

    pub(super) fn cancel_air_mission(
        &mut self,
        mission_id: u32,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let side_id = self.offensive_strike_side()?;
        self.open_strike_plan()?;
        let plan = self.strike_plan.as_mut().expect("checked open");
        let index = plan
            .missions
            .iter()
            .position(|mission| mission.id == mission_id)
            .ok_or_else(|| RuleError::new("unknownMission", "No such air mission"))?;
        let mission = plan.missions.remove(index);
        let points = self.side_air_points_mut(&side_id);
        match mission.source {
            AirPointSource::Tactical => points.tactical += 1,
            AirPointSource::BonusTactical => points.bonus_tactical += 1,
            AirPointSource::Operational => points.operational += 1,
        }
        Ok(vec![GameEvent::AirMissionCancelled {
            side_id,
            mission_id,
        }])
    }

    pub(super) fn resolve_air_strikes(&mut self) -> Result<Vec<GameEvent>, RuleError> {
        self.offensive_strike_side()?;
        self.open_strike_plan()?;
        Ok(self.resolve_missions())
    }

    // ------------------------------------------------------------------ preview

    /// Read-only preview of the Air Strike Segment for the phasing side.
    pub fn air_strike_options(&self) -> Result<AirStrikeOptions, RuleError> {
        let side_id = self.offensive_strike_side()?;
        let airspace = self.airspace_map(&side_id);
        let plan = self.strike_plan.as_ref();
        let targeted: Vec<&UnitId> = plan
            .into_iter()
            .flat_map(|plan| &plan.missions)
            .filter_map(|mission| match &mission.kind {
                AirMissionKind::Strike { unit_ids } => Some(unit_ids),
                AirMissionKind::Interdiction => None,
            })
            .flatten()
            .collect();
        let mut tactical_hexes = Vec::new();
        let mut friendly_hexes = Vec::new();
        let mut targets = Vec::new();
        for hex in &self.scenario.map.hexes {
            let hex_airspace = airspace.of(&hex.id.0);
            if is_tactical_airspace(hex_airspace) {
                tactical_hexes.push(hex.id.clone());
            }
            if hex_airspace == Airspace::Friendly {
                friendly_hexes.push(hex.id.clone());
            }
            let units: Vec<StrikeTargetUnit> = self
                .units
                .values()
                .filter(|unit| {
                    unit.definition.side_id != side_id
                        && matches!(&unit.location, UnitLocation::Hex { hex_id } if *hex_id == hex.id)
                })
                .map(|unit| StrikeTargetUnit {
                    unit_id: unit.id().clone(),
                    steps: unit.step_count(),
                    modifier: self.strike_modifier(&side_id, unit, hex, hex_airspace),
                    headquarters: unit.is_headquarters(),
                    already_targeted: targeted.contains(&unit.id()),
                })
                .collect();
            if units.is_empty() {
                continue;
            }
            let strikes_here = plan.map_or(0, |plan| {
                plan.missions
                    .iter()
                    .filter(|mission| {
                        mission.hex_id == hex.id
                            && matches!(mission.kind, AirMissionKind::Strike { .. })
                    })
                    .count()
            });
            targets.push(StrikeTargetHex {
                hex_id: hex.id.clone(),
                airspace: hex_airspace,
                tactical_allowed: is_tactical_airspace(hex_airspace),
                strikes_remaining: AIR_STRIKES_PER_HEX.saturating_sub(strikes_here) as u8,
                units,
            });
        }
        Ok(AirStrikeOptions {
            tactical_hexes,
            friendly_hexes,
            targets,
        })
    }

    // ------------------------------------------------------------------ resolution

    /// 23.2.4: every mission is committed before any is resolved; they then
    /// resolve in the order committed.
    fn resolve_missions(&mut self) -> Vec<GameEvent> {
        let Some(plan) = self.strike_plan.as_mut() else {
            return Vec::new();
        };
        plan.resolved = true;
        let side_id = plan.side_id.clone();
        let missions = plan.missions.clone();
        let mut events = Vec::new();
        for mission in missions {
            match &mission.kind {
                AirMissionKind::Interdiction => {
                    self.air_interdiction_zones.push(AirInterdictionZone {
                        side_id: side_id.clone(),
                        hex_id: mission.hex_id.clone(),
                    });
                    events.push(GameEvent::AirInterdictionZonePlaced {
                        side_id: side_id.clone(),
                        hex_id: mission.hex_id.clone(),
                    });
                }
                AirMissionKind::Strike { unit_ids } => {
                    let mut consequences = Vec::new();
                    let resolution =
                        self.resolve_strike(&side_id, &mission.hex_id, unit_ids, &mut consequences);
                    if let Some(entry) = self
                        .strike_plan
                        .as_mut()
                        .and_then(|plan| plan.missions.iter_mut().find(|m| m.id == mission.id))
                    {
                        entry.resolution = Some(resolution.clone());
                    }
                    // The roll is reported before its consequences.
                    events.push(GameEvent::AirStrikeResolved {
                        side_id: side_id.clone(),
                        mission_id: mission.id,
                        hex_id: mission.hex_id.clone(),
                        unit_ids: unit_ids.clone(),
                        resolution,
                    });
                    events.extend(consequences);
                }
            }
        }
        events
    }

    fn resolve_strike(
        &mut self,
        side_id: &SideId,
        hex_id: &HexId,
        unit_ids: &[UnitId],
        events: &mut Vec<GameEvent>,
    ) -> StrikeResolution {
        let hex = self
            .scenario
            .map
            .hexes
            .iter()
            .find(|hex| hex.id == *hex_id)
            .expect("validated when planned")
            .clone();
        let airspace = self.airspace_map(side_id).of(&hex_id.0);
        let targets: Vec<UnitId> = unit_ids
            .iter()
            .filter(|id| self.units.contains_key(*id))
            .cloned()
            .collect();
        // 23.3.1: when targets would get different modifiers, the worse set applies.
        let modifier = targets
            .iter()
            .map(|id| self.strike_modifier(side_id, &self.units[id], &hex, airspace))
            .min()
            .unwrap_or(0);
        let die_roll = self.dice.d6();
        let modified_roll = die_roll as i8 + modifier;
        let result = strike_table(modified_roll);
        let enemy_present_before = self.enemy_units_in(side_id, hex_id) > 0;

        match result {
            StrikeResult::NoEffect => {}
            StrikeResult::Disrupted => {
                for id in &targets {
                    self.disrupt(id, events);
                }
            }
            StrikeResult::StepLoss => {
                // 23.3.2: the first named unit loses the step; the rest are Disrupted.
                for (index, id) in targets.iter().enumerate() {
                    let is_hq = self.units[id].is_headquarters();
                    if index == 0 && !is_hq {
                        self.lose_step(id, hex_id, events);
                    } else {
                        self.disrupt(id, events);
                    }
                }
            }
        }

        // 23.3.2: clearing a hex with Offensive Strikes leaves a Breakthrough Marker.
        if enemy_present_before
            && self.enemy_units_in(side_id, hex_id) == 0
            && !self.breakthrough_markers.contains(hex_id)
        {
            self.breakthrough_markers.push(hex_id.clone());
            events.push(GameEvent::BreakthroughMarkerPlaced {
                hex_id: hex_id.clone(),
            });
        }

        StrikeResolution {
            die_roll,
            modifier,
            modified_roll,
            result,
        }
    }

    /// Strike Table die roll modifiers for one target unit.
    fn strike_modifier(
        &self,
        side_id: &SideId,
        unit: &UnitState,
        hex: &MapHex,
        airspace: Airspace,
    ) -> i8 {
        let mut modifier = 0;
        if unit.train_status.is_some() {
            // 13.4.5: no terrain benefit, and +1 against units under a train marker.
            modifier += 1;
        } else {
            modifier += match (&hex.city, hex.terrain) {
                (Some(city), _) if matches!(city.kind, CityKind::Major | CityKind::Key) => -2,
                (Some(_), _) => -1,
                (None, Terrain::Forest | Terrain::Rough | Terrain::Mountain) => -1,
                _ => 0,
            };
        }
        // 11.8.4: Air Strikes into friendly Airspace +1, into enemy Airspace -1.
        modifier += match airspace {
            Airspace::Friendly => 1,
            Airspace::Contested => 0,
            Airspace::Enemy => -1,
        };
        // 35.7 (2): WP Strikes on the turn of Surprise add one.
        if side_id.0 == "warsawPact"
            && self.scenario.battle_planning_rules.air_power.surprise_turn == Some(self.game_turn)
        {
            modifier += 1;
        }
        modifier
    }

    /// 25.6.4: Disrupted (Suppressed for HQs); a Disrupted unit loses any train marker.
    pub(crate) fn disrupt(&mut self, unit_id: &UnitId, events: &mut Vec<GameEvent>) {
        let Some(unit) = self.units.get_mut(unit_id) else {
            return;
        };
        let marker = if unit.is_headquarters() {
            Disruption::Suppressed
        } else {
            Disruption::Disrupted
        };
        if unit.disruption != Some(marker) {
            unit.disruption = Some(marker);
            events.push(GameEvent::UnitDisruptionChanged {
                unit_id: unit_id.clone(),
                disruption: Some(marker),
            });
        }
        if unit.train_status.take().is_some() {
            events.push(GameEvent::TrainStatusChanged {
                unit_id: unit_id.clone(),
                status: None,
            });
        }
    }

    /// Flips a two-step unit to its reduced side and Disrupts it, or eliminates
    /// a unit on its last step (23.3.2).
    pub(crate) fn lose_step(
        &mut self,
        unit_id: &UnitId,
        hex_id: &HexId,
        events: &mut Vec<GameEvent>,
    ) {
        if self.remove_step(unit_id, hex_id, events) {
            self.disrupt(unit_id, events);
        }
    }

    /// Removes one step: flips a multi-step unit or eliminates a unit on its
    /// last step. Returns whether the unit is still in play.
    pub(crate) fn remove_step(
        &mut self,
        unit_id: &UnitId,
        hex_id: &HexId,
        events: &mut Vec<GameEvent>,
    ) -> bool {
        let Some(unit) = self.units.get_mut(unit_id) else {
            return false;
        };
        if unit.step_count() > 1 {
            unit.strength_step_index += 1;
            events.push(GameEvent::UnitStepLost {
                unit_id: unit_id.clone(),
                strength_step_index: unit.strength_step_index,
            });
            true
        } else {
            self.units.remove(unit_id);
            self.eliminated_units.push(unit_id.clone());
            events.push(GameEvent::UnitEliminated {
                unit_id: unit_id.clone(),
                hex_id: hex_id.clone(),
            });
            false
        }
    }

    // ------------------------------------------------------------------ helpers

    fn offensive_strike_side(&self) -> Result<SideId, RuleError> {
        let Some(step) = self.current_step() else {
            return Err(RuleError::game_complete());
        };
        if step.phase_id.0 != "offensiveStrike" {
            return Err(RuleError::new(
                "wrongPhase",
                "This command is available only during the Offensive Strike Phase",
            ));
        }
        match &step.actor {
            PhaseActor::Side { side_id } => Ok(side_id.clone()),
            PhaseActor::All => Err(RuleError::new(
                "wrongActor",
                "The Offensive Strike Phase must have one acting side",
            )),
        }
    }

    fn open_strike_plan(&self) -> Result<(), RuleError> {
        match &self.strike_plan {
            None => Err(RuleError::new(
                "strikePlanUnavailable",
                "No Offensive Strike Phase is in progress",
            )),
            Some(plan) if plan.resolved => Err(RuleError::new(
                "strikesResolved",
                "This phase's air missions have already been resolved",
            )),
            Some(_) => Ok(()),
        }
    }

    pub(crate) fn map_hex(&self, hex_id: &HexId) -> Result<MapHex, RuleError> {
        self.scenario
            .map
            .hexes
            .iter()
            .find(|hex| hex.id == *hex_id)
            .cloned()
            .ok_or_else(|| RuleError::new("unknownHex", format!("Unknown hex: {}", hex_id.0)))
    }

    pub(crate) fn enemy_units_in(&self, side_id: &SideId, hex_id: &HexId) -> usize {
        self.units
            .values()
            .filter(|unit| {
                unit.definition.side_id != *side_id
                    && matches!(&unit.location, UnitLocation::Hex { hex_id: at } if at == hex_id)
            })
            .count()
    }

    fn side_air_points_mut(&mut self, side_id: &SideId) -> &mut crate::model::AirPoints {
        self.air_points
            .iter_mut()
            .find(|points| points.side_id == *side_id)
            .expect("every scenario side has an Air Point record")
    }

    /// Spends one Air Point of the requested kind, drawing Tactical points from
    /// this turn's pool before the one-time bonus.
    fn take_air_point(
        &mut self,
        side_id: &SideId,
        kind: AirPointKind,
        airspace: Airspace,
    ) -> Result<AirPointSource, RuleError> {
        // 23.1.3: Tactical Air Points never strike into enemy Airspace.
        if kind == AirPointKind::Tactical && !is_tactical_airspace(airspace) {
            return Err(RuleError::new(
                "enemyAirspace",
                "Tactical Air Points may be used only in friendly or contested Airspace",
            ));
        }
        let points = self.side_air_points_mut(side_id);
        let source = match kind {
            AirPointKind::Tactical if points.tactical > 0 => {
                points.tactical -= 1;
                AirPointSource::Tactical
            }
            AirPointKind::Tactical if points.bonus_tactical > 0 => {
                points.bonus_tactical -= 1;
                AirPointSource::BonusTactical
            }
            AirPointKind::Operational if points.operational > 0 => {
                points.operational -= 1;
                AirPointSource::Operational
            }
            _ => {
                return Err(RuleError::new(
                    "noAirPoints",
                    "No Air Points of that kind remain this turn",
                ))
            }
        };
        Ok(source)
    }

    fn push_mission(
        &mut self,
        side_id: SideId,
        hex_id: HexId,
        kind: AirMissionKind,
        source: AirPointSource,
    ) -> Result<Vec<GameEvent>, RuleError> {
        let plan = self.strike_plan.as_mut().expect("checked open");
        let mission = AirMission {
            id: plan.next_mission_id,
            hex_id,
            kind,
            source,
            resolution: None,
        };
        plan.next_mission_id += 1;
        plan.missions.push(mission.clone());
        Ok(vec![GameEvent::AirMissionPlanned { side_id, mission }])
    }
}
