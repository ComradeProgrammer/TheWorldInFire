use serde::{Deserialize, Serialize};

use crate::model::{
    AirMission, AirPoints, BattleReport, Disruption, HexId, MovementMode, PendingAdvance,
    PhaseDefinition, PlannedMovement, SideId, StrikeResolution, TrainStatus, UnitId, UnitLocation,
    UnitState, UnitSupplyState,
};

/// Supply result recorded for one unit during automatic pre-battle resolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitSupplyCheck {
    /// Unit whose supply state was checked.
    pub unit_id: UnitId,
    /// Authoritative supply state after the check.
    pub supply: UnitSupplyState,
}

/// A fact emitted after the authoritative game state changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GameEvent {
    /// The indicated phase has finished.
    PhaseEnded {
        /// Game turn on which the phase ended.
        game_turn: u16,
        /// Definition of the phase that ended.
        step: PhaseDefinition,
    },
    /// The indicated phase has become current.
    PhaseStarted {
        /// Game turn on which the phase started.
        game_turn: u16,
        /// Definition of the phase that started.
        step: PhaseDefinition,
    },
    /// A new game turn has begun.
    GameTurnStarted {
        /// Newly active game turn.
        game_turn: u16,
    },
    /// Scenario-scheduled units have entered authoritative state.
    ReinforcementsArrived {
        /// Game turn on which the units arrived.
        game_turn: u16,
        /// Complete states of the newly arrived units.
        units: Vec<UnitState>,
    },
    /// The acting side's automatic pre-battle supply pass has completed.
    PreBattleSupplyChecked {
        /// Game turn on which the check occurred.
        game_turn: u16,
        /// Side whose units were checked.
        side_id: SideId,
        /// Supply state recorded for each of the side's units in play.
        units: Vec<UnitSupplyCheck>,
    },
    /// A battle-plan resupply target has been selected or replaced.
    ResupplyTargetSet {
        /// Side making the plan.
        side_id: SideId,
        /// Friendly unit identifying the stack, or `None` after cancellation.
        unit_id: Option<UnitId>,
    },
    /// An objective hex has been added to or removed from the battle plan.
    AttackTargetSet {
        /// Side making the plan.
        side_id: SideId,
        /// Objective hex changed by the command.
        hex_id: HexId,
        /// Whether the objective is now selected.
        selected: bool,
    },
    /// A unit completed an authoritative movement order.
    UnitMoved {
        /// Unit that moved.
        unit_id: UnitId,
        /// Location before movement.
        from: UnitLocation,
        /// Destination after movement.
        to: HexId,
        /// Movement system used.
        mode: MovementMode,
        /// Movement points or rail hexes consumed.
        cost: u16,
        /// Core-selected route, including the destination but not the origin.
        path: Vec<HexId>,
    },
    /// The most recent movement order for a unit was reversed.
    UnitMovementUndone {
        /// Movement removed from the plan.
        movement: PlannedMovement,
        /// Authoritative location restored by the undo.
        restored_location: UnitLocation,
    },
    /// A city hex changed hands (rule 30.1).
    CityControlChanged {
        /// City hex.
        hex_id: HexId,
        /// New controller.
        controller: SideId,
        /// Whether the city is now a Free City.
        free: bool,
    },
    /// A unit's persistent rail-loading state changed.
    TrainStatusChanged {
        /// Unit whose status changed.
        unit_id: UnitId,
        /// New status, or `None` after detraining.
        status: Option<TrainStatus>,
    },
    /// The selected resupply operation was applied when planning ended.
    UnitsResupplied {
        /// Hex whose combat units were resupplied.
        hex_id: HexId,
        /// Units that received applicable supply.
        unit_ids: Vec<UnitId>,
    },
    /// Each side's Air Points were reset for the new game turn.
    AirPointsReset {
        /// Balances after the reset.
        air_points: Vec<AirPoints>,
    },
    /// An Air Point was committed to a mission.
    AirMissionPlanned {
        /// Striking side.
        side_id: SideId,
        /// Committed mission.
        mission: AirMission,
    },
    /// An unresolved mission was withdrawn and its Air Point refunded.
    AirMissionCancelled {
        /// Striking side.
        side_id: SideId,
        /// Withdrawn mission.
        mission_id: u32,
    },
    /// An Air Strike was rolled on the Strike Table.
    AirStrikeResolved {
        /// Striking side.
        side_id: SideId,
        /// Resolved mission.
        mission_id: u32,
        /// Target hex.
        hex_id: HexId,
        /// Targeted units.
        unit_ids: Vec<UnitId>,
        /// Dice outcome.
        resolution: StrikeResolution,
    },
    /// An Air Interdiction Zone became active.
    AirInterdictionZonePlaced {
        /// Side that placed it.
        side_id: SideId,
        /// Marked hex.
        hex_id: HexId,
    },
    /// A side's Air Interdiction Zones were removed (23.8.2).
    AirInterdictionZonesRemoved {
        /// Side that had placed them.
        side_id: SideId,
        /// Marked hexes removed.
        hex_ids: Vec<HexId>,
    },
    /// A unit became Disrupted or Suppressed, or recovered (`None`).
    UnitDisruptionChanged {
        /// Unit.
        unit_id: UnitId,
        /// New marker, if any.
        disruption: Option<Disruption>,
    },
    /// A unit lost a step and flipped to its reduced side.
    UnitStepLost {
        /// Unit.
        unit_id: UnitId,
        /// New active step index.
        strength_step_index: usize,
    },
    /// A unit lost its last step and left play.
    UnitEliminated {
        /// Unit.
        unit_id: UnitId,
        /// Hex it occupied.
        hex_id: HexId,
    },
    /// A Strike cleared a hex of enemy units (25.9.1).
    BreakthroughMarkerPlaced {
        /// Hex.
        hex_id: HexId,
    },
    /// Breakthrough Markers were removed at the end of the Reserve Phase.
    BreakthroughMarkersRemoved {
        /// Hexes cleared.
        hex_ids: Vec<HexId>,
    },
    /// The scenario removed a unit from play.
    UnitWithdrawn {
        /// Game turn of the withdrawal.
        game_turn: u16,
        /// Unit removed.
        unit_id: UnitId,
    },
    /// A battle was fought (25.1); consequences follow as separate events.
    BattleResolved {
        /// Attacking side.
        side_id: SideId,
        /// Strengths, odds, roll, and result.
        report: BattleReport,
    },
    /// A defending unit retreated after combat (25.7).
    UnitRetreated {
        /// Unit.
        unit_id: UnitId,
        /// Objective hex it left.
        from: HexId,
        /// Hexes entered, ending where it stopped.
        path: Vec<HexId>,
    },
    /// The Attacker may advance into a cleared Objective hex.
    AdvanceOffered {
        /// Advance decision now pending.
        pending: PendingAdvance,
    },
    /// Attacking units advanced into the Objective hex (25.8.1).
    UnitsAdvanced {
        /// Attacking side.
        side_id: SideId,
        /// Objective hex.
        hex_id: HexId,
        /// Units that advanced.
        unit_ids: Vec<UnitId>,
    },
    /// The scenario has reached its final turn and phase.
    GameCompleted {
        /// Game turn on which play ended.
        game_turn: u16,
    },
}
