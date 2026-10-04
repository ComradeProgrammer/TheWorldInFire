use serde::{Deserialize, Serialize};

use super::{HexId, SideId, UnitId};

/// Combat Results Table columns, weakest to strongest.
pub const ODDS_COLUMNS: [&str; 13] = [
    "1:4", "1:3", "1:2", "1:1", "2:1", "3:1", "4:1", "5:1", "6:1", "7:1", "8:1", "9:1", "10:1",
];

/// One cell of the Combat Results Table, e.g. `A1/D1R1` (25.6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatResult {
    /// Printed code, e.g. `-/CAD1R1`.
    pub code: String,
    /// Attacker steps lost (A1).
    pub attacker_steps: u8,
    /// Attacking units are Disrupted (`*` on the attacker side).
    pub attacker_disrupted: bool,
    /// The Defender Counterattacks first (CA).
    pub counterattack: bool,
    /// Defender steps lost (D#).
    pub defender_steps: u8,
    /// Defending units are Disrupted (`*` on the defender side).
    pub defender_disrupted: bool,
    /// Hexes the Defender must retreat (R#).
    pub retreat: u8,
}

/// Why a unit's printed strength was adjusted for a battle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StrengthModifier {
    /// Halved: Disrupted or Suppressed.
    Disrupted,
    /// Halved: Out of Combat Supply.
    OutOfCombatSupply,
    /// Halved: an Armored unit attacking into a City or Mountain hex.
    ArmorIntoCityOrMountain,
    /// Reduced by a quarter: attacking across a Minor River hexside.
    MinorRiver,
    /// Halved: attacking across a Major River hexside.
    MajorRiver,
    /// Doubled: a Soft unit defending in Forest, Rough, Mountain, or a City.
    SoftUnitCover,
    /// Counts only because no Maneuver unit defends the hex (25.2.7).
    ProvisionalDefense,
    /// An adjustment made by another plugin through a filter, serialized as
    /// its own name, such as `myMod.nightAttack`.
    #[serde(untagged)]
    Custom(String),
}

/// One unit's contribution to a battle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitStrength {
    /// Unit.
    pub unit_id: UnitId,
    /// Printed Attack or Defense Strength of its current step.
    pub printed: u16,
    /// Adjusted strength in sixty-fourths, keeping fractions exact.
    pub adjusted_64ths: u32,
    /// Adjustments applied, in order.
    pub modifiers: Vec<StrengthModifier>,
}

/// Why the odds column shifted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColumnShiftReason {
    /// Primary terrain of the Objective hex.
    Terrain,
    /// Surrounded by the Attacker's units or ZOCs but next to other defenders.
    FlankAttack,
    /// Surrounded by the Attacker's units or ZOCs and isolated.
    ConcentricAttack,
    /// Warsaw Pact attack on the turn of Surprise.
    Surprise,
    /// An HQ committed Offensive Support (25.4).
    OffensiveSupport,
    /// A shift added by another plugin through a filter, serialized as its own
    /// name, such as `myMod.nightAttack`.
    #[serde(untagged)]
    Custom(String),
}

/// One column shift.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnShift {
    /// Reason.
    pub reason: ColumnShiftReason,
    /// Columns; negative favours the Defender.
    pub shift: i8,
}

/// Strengths and odds for a proposed or resolved battle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleOdds {
    /// Attacking units.
    pub attackers: Vec<UnitStrength>,
    /// Defending units that add their Defense Strength.
    pub defenders: Vec<UnitStrength>,
    /// Organic Defense Strength of an enemy Free City in the hex.
    pub city_defense: u16,
    /// Total Adjusted Attack Strength, rounded down.
    pub total_attack: u16,
    /// Total Adjusted Defense Strength, rounded up.
    pub total_defense: u16,
    /// Index into [`ODDS_COLUMNS`] before shifts.
    pub basic_column: usize,
    /// Column shifts that apply.
    pub shifts: Vec<ColumnShift>,
    /// Net shift after the two-column limit.
    pub net_shift: i8,
    /// Index into [`ODDS_COLUMNS`] used to resolve the battle.
    pub final_column: usize,
    /// Printed odds, e.g. `3:1`.
    pub final_odds: String,
    /// The six possible results on the final column, die rolls 1 to 6.
    pub possible_results: Vec<String>,
}

/// One Counterattack roll (25.6.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CounterattackRoll {
    /// Defending unit whose step Counterattacked.
    pub unit_id: UnitId,
    /// Attacking unit targeted.
    pub target_unit_id: UnitId,
    /// Die roll.
    pub die_roll: u8,
    /// Whether the target was Disrupted.
    pub disrupted: bool,
}

/// A battle fought this Combat Phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleReport {
    /// Identifier unique within the phase.
    pub id: u32,
    /// Objective hex.
    pub hex_id: HexId,
    /// Committed attacking units.
    pub attacking_unit_ids: Vec<UnitId>,
    /// Strengths and odds; `None` for a Breakthrough advance with no defenders.
    pub odds: Option<BattleOdds>,
    /// Combat die roll.
    pub die_roll: Option<u8>,
    /// Combat result.
    pub result: Option<CombatResult>,
    /// Counterattack rolls, when the result included CA.
    pub counterattacks: Vec<CounterattackRoll>,
    /// HQ that gave Offensive Support, if any.
    pub supporting_hq_id: Option<UnitId>,
}

/// A cleared Objective hex the Attacker may advance into (25.8.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingAdvance {
    /// Battle that cleared the hex.
    pub battle_id: u32,
    /// Objective hex.
    pub hex_id: HexId,
    /// Surviving attackers that may advance.
    pub eligible_unit_ids: Vec<UnitId>,
    /// Advancing conquers an enemy Free City.
    pub conquers_free_city: bool,
}

/// Battles and restrictions of the current Combat Phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatState {
    /// Attacking side.
    pub side_id: SideId,
    /// Battles in the order fought.
    pub battles: Vec<BattleReport>,
    /// Units that have attacked; none may attack twice (25.2).
    pub attacked_unit_ids: Vec<UnitId>,
    /// Hexes that have been attacked; none may be attacked twice.
    pub attacked_hex_ids: Vec<HexId>,
    /// Defenders with no Defense Strength for the rest of the phase (25.6.5).
    pub engaged_unit_ids: Vec<UnitId>,
    /// HQs that have given Offensive Support this phase; each may do so once (25.4.1).
    pub supporting_hq_ids: Vec<UnitId>,
    /// Advance decision the Attacker must make before the next battle.
    pub pending_advance: Option<PendingAdvance>,
}

/// A hex the attacking side may attack now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatObjective {
    /// Objective hex.
    pub hex_id: HexId,
    /// Adjacent units able to attack it.
    pub eligible_unit_ids: Vec<UnitId>,
    /// A WP Battle Marker hex that must be attacked (25.0).
    pub mandatory: bool,
    /// Empty hex holding a Breakthrough Marker: the battle is an advance only.
    pub breakthrough_only: bool,
    /// HQs able to give Offensive Support if every eligible unit attacks.
    pub support_hq_ids: Vec<UnitId>,
}

/// Read-only preview of the Combat Phase choices.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatOptions {
    /// Hexes that may be attacked now.
    pub objectives: Vec<CombatObjective>,
    /// Marked objectives that still must be attacked before the phase may end.
    pub mandatory_remaining: Vec<HexId>,
}
