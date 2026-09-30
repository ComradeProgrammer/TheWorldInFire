use serde::{Deserialize, Serialize};

use super::{HexId, SideId, UnitId};

/// Kind of Air Point a player chooses to spend (23.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AirPointKind {
    /// Short-range point; only for targets in friendly or contested Airspace.
    Tactical,
    /// Long-range point; any target on the map, and the only kind that may strike an HQ.
    Operational,
}

/// Pool an Air Point was actually drawn from, so cancelling refunds it correctly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AirPointSource {
    /// This turn's Tactical Air Points.
    Tactical,
    /// The scenario's one-time extra Tactical Air Point.
    BonusTactical,
    /// This turn's Operational Air Points.
    Operational,
}

impl AirPointSource {
    /// Maps the consumed Air Point source to its player-selectable kind.
    ///
    /// # Parameters
    ///
    /// - `self`: Source pool to classify: recurring Tactical, bonus Tactical,
    ///   or Operational.
    ///
    /// # Returns
    ///
    /// [`AirPointKind::Tactical`] for either Tactical pool, or
    /// [`AirPointKind::Operational`] for the Operational pool.
    pub fn kind(self) -> AirPointKind {
        match self {
            Self::Tactical | Self::BonusTactical => AirPointKind::Tactical,
            Self::Operational => AirPointKind::Operational,
        }
    }
}

/// Air Points held by one side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirPoints {
    /// Side holding the points.
    pub side_id: SideId,
    /// Tactical Air Points left this turn; unused points are lost at the next reset.
    pub tactical: u16,
    /// Operational Air Points left this turn.
    pub operational: u16,
    /// One-time extra Tactical Air Points that may be spent on any turn.
    pub bonus_tactical: u16,
}

/// Airspace of a hex from one side's point of view (11.0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Airspace {
    /// Within five hexes of a friendly source and not of an enemy one.
    Friendly,
    /// Within five hexes of both sides' sources (or of neither).
    Contested,
    /// Within five hexes of an enemy source only.
    Enemy,
}

/// Lasting effect of an enemy Strike or battle on a unit (25.6.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Disruption {
    /// A Maneuver unit that may only use Minimum movement and fights at half strength.
    Disrupted,
    /// The HQ or Artillery form of disruption, removed in the Unsuppression step.
    Suppressed,
}

/// Row of the Strike Table reached by a modified die roll.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StrikeResult {
    /// "—": no effect.
    NoEffect,
    /// "*": every target is Disrupted (HQs Suppressed).
    Disrupted,
    /// "D1*": one target step is lost and the rest are Disrupted (HQs Suppressed).
    StepLoss,
}

/// Dice outcome of one resolved Air Strike.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrikeResolution {
    /// Natural die roll, 1 to 6.
    pub die_roll: u8,
    /// Sum of the die roll modifiers applied.
    pub modifier: i8,
    /// Die roll after modifiers.
    pub modified_roll: i8,
    /// Strike Table result.
    pub result: StrikeResult,
}

/// What an air mission does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AirMissionKind {
    /// An Air Strike on up to two enemy steps in one hex (23.3).
    Strike {
        /// Targeted units; the first absorbs a step loss.
        unit_ids: Vec<UnitId>,
    },
    /// An Air Interdiction Zone centred on the hex (23.8).
    Interdiction,
}

/// One Air Point committed during the Air Strike Segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirMission {
    /// Identifier unique within the strike plan.
    pub id: u32,
    /// Target hex.
    pub hex_id: HexId,
    /// Strike or interdiction.
    pub kind: AirMissionKind,
    /// Pool the Air Point was drawn from.
    pub source: AirPointSource,
    /// Dice outcome once resolved; always `None` for interdiction.
    pub resolution: Option<StrikeResolution>,
}

/// Air missions committed by the phasing side during its Offensive Strike Phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrikePlan {
    /// Game turn of the plan.
    pub game_turn: u16,
    /// Side making the strikes.
    pub side_id: SideId,
    /// Missions in the order they were committed.
    pub missions: Vec<AirMission>,
    /// Whether the missions have been resolved; no further missions may be added.
    pub resolved: bool,
    /// Identifier for the next committed mission.
    pub next_mission_id: u32,
}

/// An active Air Interdiction Zone: the marked hex and its six neighbours.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirInterdictionZone {
    /// Side that placed the marker; the zone affects only the other side.
    pub side_id: SideId,
    /// Marked hex.
    pub hex_id: HexId,
}

/// Air power granted to one side by the scenario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideAirPower {
    /// Tactical Air Points received each Joint Reinforcement Phase.
    pub tactical_per_turn: u16,
    /// Operational Air Points received each Joint Reinforcement Phase.
    pub operational_per_turn: u16,
    /// One-time extra Tactical Air Points usable on any turn.
    pub bonus_tactical: u16,
}

/// Scenario air power and surprise settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AirPowerRules {
    /// Warsaw Pact air power.
    pub warsaw_pact: SideAirPower,
    /// NATO air power.
    pub nato: SideAirPower,
    /// Game turn on which NATO is Surprised (35.7), if any.
    pub surprise_turn: Option<u16>,
}

impl AirPowerRules {
    /// Returns the scenario's Air Point allocation for one side.
    ///
    /// # Parameters
    ///
    /// - `side_id`: Side to query. `warsawPact` selects the Warsaw Pact allocation;
    ///   every other identifier selects the NATO allocation.
    ///
    /// # Returns
    ///
    /// A shared [`SideAirPower`] configuration containing recurring and one-time
    /// allocations. Remaining in-game points are stored separately in [`AirPoints`].
    pub fn for_side(&self, side_id: &SideId) -> &SideAirPower {
        if side_id.0 == "warsawPact" {
            &self.warsaw_pact
        } else {
            &self.nato
        }
    }
}

/// A unit that may be named as an Air Strike target, with its current modifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrikeTargetUnit {
    /// Unit.
    pub unit_id: UnitId,
    /// Steps the unit contributes toward the two-step limit.
    pub steps: u16,
    /// Die roll modifier that would apply if it were struck now.
    pub modifier: i8,
    /// HQs may be struck only alone and only with Operational Air Points.
    pub headquarters: bool,
    /// Already named in another mission this segment.
    pub already_targeted: bool,
}

/// One enemy-occupied hex the phasing side could strike.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrikeTargetHex {
    /// Hex.
    pub hex_id: HexId,
    /// Airspace from the striking side's point of view.
    pub airspace: Airspace,
    /// Whether Tactical Air Points may be spent here.
    pub tactical_allowed: bool,
    /// Further Air Strikes the hex may still receive this segment.
    pub strikes_remaining: u8,
    /// Enemy units in the hex.
    pub units: Vec<StrikeTargetUnit>,
}

/// Read-only preview of the phasing side's Air Strike Segment choices.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirStrikeOptions {
    /// Hexes where Tactical Air Points may be spent (friendly or contested Airspace).
    pub tactical_hexes: Vec<HexId>,
    /// Hexes in friendly Airspace for the striking side.
    pub friendly_hexes: Vec<HexId>,
    /// Enemy-occupied hexes with targetable units.
    pub targets: Vec<StrikeTargetHex>,
}
