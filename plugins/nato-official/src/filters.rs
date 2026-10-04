//! Extension points of the official rules.
//!
//! The NATO rules compute each value below themselves, then pass it through
//! every other loaded plugin whose manifest lists the filter name. A plugin
//! adjusts the value in its `filter` handler and returns it; plugins run in
//! load order. Filters are read-only: they may not change state or roll dice.
//!
//! Inputs and values are the JSON forms of the types named on each constant.

use serde::{Deserialize, Serialize};

use crate::model::{HexId, SideId, UnitId};

/// One attacking unit's strength in a battle.
///
/// Input: [`UnitStrengthInput`]. Value: [`crate::UnitStrength`] after the
/// printed modifiers (disruption, supply, terrain, rivers).
pub const ATTACK_STRENGTH: &str = "nato.combat.attackStrength";

/// One defending unit's strength in a battle.
///
/// Input: [`UnitStrengthInput`]. Value: [`crate::UnitStrength`] after the
/// printed modifiers.
pub const DEFENSE_STRENGTH: &str = "nato.combat.defenseStrength";

/// Column shifts of a battle before the two-column limit is applied.
///
/// Input: [`ColumnShiftsInput`]. Value: a list of [`crate::ColumnShift`].
pub const COLUMN_SHIFTS: &str = "nato.combat.columnShifts";

/// The Combat Results Table outcome of a battle.
///
/// Input: [`CombatResultInput`]. Value: [`crate::CombatResult`].
pub const COMBAT_RESULT: &str = "nato.combat.result";

/// The Strike Table die-roll modifier of an Air Strike.
///
/// Input: [`StrikeModifierInput`]. Value: the modifier, an integer.
pub const STRIKE_MODIFIER: &str = "nato.air.strikeModifier";

/// Context of [`ATTACK_STRENGTH`] and [`DEFENSE_STRENGTH`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitStrengthInput {
    /// Attacking side.
    pub side_id: SideId,
    /// Objective hex.
    pub objective: HexId,
    /// Unit whose strength is filtered.
    pub unit_id: UnitId,
}

/// Context of [`COLUMN_SHIFTS`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnShiftsInput {
    /// Attacking side.
    pub side_id: SideId,
    /// Objective hex.
    pub objective: HexId,
    /// Committed attacking units.
    pub attacking_unit_ids: Vec<UnitId>,
    /// Whether an HQ gives Offensive Support.
    pub offensive_support: bool,
}

/// Context of [`COMBAT_RESULT`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatResultInput {
    /// Attacking side.
    pub side_id: SideId,
    /// Objective hex.
    pub objective: HexId,
    /// Index into [`crate::ODDS_COLUMNS`] used to resolve the battle.
    pub final_column: usize,
    /// Combat die roll.
    pub die_roll: u8,
}

/// Context of [`STRIKE_MODIFIER`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrikeModifierInput {
    /// Striking side.
    pub side_id: SideId,
    /// Target hex.
    pub hex_id: HexId,
    /// Targeted units.
    pub unit_ids: Vec<UnitId>,
}
