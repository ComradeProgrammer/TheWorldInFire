#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

use super::{HexId, NationId, SideId, UnitId};

/// Stable identifier of one air squadron counter.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AirUnitId(pub String);

/// Stable identifier of one off-map airbase.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AirBaseId(pub String);

/// The mission family an air counter may perform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AirUnitKind {
    Fighter,
    FighterBomber,
    Aew,
}

/// Printed values on one side of a two-step air counter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirUnitStep {
    pub air_combat: i8,
    pub evasion: i8,
    pub strike_modifier: i8,
    pub combat_radius: u8,
    pub aew_modifier: i8,
    pub aew_radius: u8,
}

/// Scenario-defined identity and capabilities of an air squadron.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirUnitDefinition {
    pub id: AirUnitId,
    pub name: String,
    pub side_id: SideId,
    pub nation_id: NationId,
    pub kind: AirUnitKind,
    pub base_id: AirBaseId,
    pub steps: [AirUnitStep; 2],
}

/// Whether an air counter may receive another mission this game turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AirReadiness {
    Ready,
    Aborted,
    Flown,
}

/// Mutable state of one air squadron counter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirUnitState {
    #[serde(flatten)]
    pub definition: AirUnitDefinition,
    pub strength_step_index: usize,
    pub readiness: AirReadiness,
}

impl AirUnitState {
    pub fn current_step(&self) -> &AirUnitStep {
        &self.definition.steps[self.strength_step_index]
    }

    pub fn remaining_steps(&self) -> u8 {
        (2_usize.saturating_sub(self.strength_step_index)) as u8
    }
}

/// Immutable data for one off-map airbase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirBaseDefinition {
    pub id: AirBaseId,
    pub name: String,
    pub side_id: SideId,
    pub anchor_hex_id: HexId,
    pub sortie_capacity: u8,
    pub strike_modifier: i8,
}

/// Damage and temporary suppression of an off-map airbase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirBaseState {
    #[serde(flatten)]
    pub definition: AirBaseDefinition,
    pub damage: u8,
    pub suppressed_through_turn: Option<u16>,
}

impl AirBaseState {
    pub fn effective_capacity(&self, game_turn: u16) -> u8 {
        if self.damage >= 2
            || self
                .suppressed_through_turn
                .is_some_and(|turn| game_turn <= turn)
        {
            0
        } else if self.damage == 1 {
            self.definition.sortie_capacity.div_ceil(2)
        } else {
            self.definition.sortie_capacity
        }
    }
}

/// A mission committed during Battle Planning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AirMissionAssignment {
    AirSuperiority {
        center_hex_id: HexId,
    },
    GroundStrike {
        hex_id: HexId,
        unit_ids: Vec<UnitId>,
    },
    AirBaseStrike {
        air_base_id: AirBaseId,
    },
    EarlyWarning {
        center_hex_id: HexId,
    },
}

impl AirMissionAssignment {
    pub fn target_hex<'a>(
        &'a self,
        bases: &'a std::collections::BTreeMap<AirBaseId, AirBaseState>,
    ) -> Option<&'a HexId> {
        match self {
            Self::AirSuperiority { center_hex_id }
            | Self::GroundStrike {
                hex_id: center_hex_id,
                ..
            }
            | Self::EarlyWarning { center_hex_id } => Some(center_hex_id),
            Self::AirBaseStrike { air_base_id } => bases
                .get(air_base_id)
                .map(|base| &base.definition.anchor_hex_id),
        }
    }
}

/// Resolution state of one planned sortie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AirSortieStatus {
    Planned,
    Cleared,
    Aborted,
    Completed,
    TargetGone,
}

/// One air counter committed to one mission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirSortie {
    pub id: u32,
    pub air_unit_id: AirUnitId,
    pub mission: AirMissionAssignment,
    pub status: AirSortieStatus,
}

/// One side's current-turn air orders.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirPlan {
    pub game_turn: u16,
    pub side_id: SideId,
    pub sorties: Vec<AirSortie>,
    pub next_sortie_id: u32,
}

impl AirPlan {
    pub fn new(game_turn: u16, side_id: SideId) -> Self {
        Self {
            game_turn,
            side_id,
            sorties: Vec::new(),
            next_sortie_id: 1,
        }
    }
}

/// Raw result read from the 1985 Air Combat Table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AirCombatResult {
    NoEffect,
    Abort,
    DamagedAbort,
    DestroyedAbort,
    DestroyedAndDamagedAbort,
}

impl AirCombatResult {
    pub fn step_losses(self) -> u8 {
        match self {
            Self::NoEffect | Self::Abort => 0,
            Self::DamagedAbort | Self::DestroyedAbort => 1,
            Self::DestroyedAndDamagedAbort => 2,
        }
    }

    pub fn aborts(self) -> bool {
        self != Self::NoEffect
    }
}

/// One aircraft's attack in an air-combat pairing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirCombatAttack {
    pub attacker_id: AirUnitId,
    pub defender_id: AirUnitId,
    pub column: i8,
    pub die_roll: u8,
    pub modifier: i8,
    pub modified_roll: i8,
    pub result: AirCombatResult,
}

/// A simultaneous fighter pairing or fighter interception.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirCombatPairing {
    pub round: u8,
    pub first: AirCombatAttack,
    pub second: AirCombatAttack,
}

/// Complete automatic air-combat record for the current turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirOperationsReport {
    pub game_turn: u16,
    pub fighter_combat: Vec<AirCombatPairing>,
    pub interceptions: Vec<AirCombatPairing>,
    pub round_limit_reached: bool,
}

/// Availability and base-capacity preview for one air counter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirUnitPlanningOption {
    pub air_unit_id: AirUnitId,
    pub unavailable: Option<crate::error::RuleError>,
}

/// Planning preview for the acting side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirPlanningOptions {
    pub side_id: SideId,
    pub units: Vec<AirUnitPlanningOption>,
    pub bases: Vec<AirBaseState>,
}

/// One ground hex and the units a fighter-bomber may name there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirGroundTarget {
    pub hex_id: HexId,
    pub unit_ids: Vec<UnitId>,
}

/// Legal target families for one air counter during planning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirMissionOptions {
    pub air_unit_id: AirUnitId,
    pub kind: AirUnitKind,
    pub center_hexes: Vec<HexId>,
    pub ground_targets: Vec<AirGroundTarget>,
    pub air_base_ids: Vec<AirBaseId>,
}
