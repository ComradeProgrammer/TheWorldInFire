//! UI-independent rules engine and domain model for OOA Wargame.
//!
//! The [`model`] module contains scenario and unit data. The crate-level types
//! execute commands, advance the turn state machine, and produce IPC-friendly
//! snapshots and events.

#![deny(missing_docs)]

mod airspace;
mod cities;
mod combat;
mod command;
mod dice;
mod engine;
mod error;
mod event;
/// Domain data for sides, phases, scenarios, and units.
pub mod model;
mod movement;
mod phase;
mod planning;
mod reserve;
mod setup;
mod strikes;
mod supply;

pub use command::{CommandOutcome, GameCommand};
pub use engine::{GameEngine, GameId, GameSnapshot, GameStatus, PendingDecision, TurnState};
pub use error::RuleError;
pub use event::{GameEvent, UnitSupplyCheck};
pub use model::{
    find_scenario, list_scenarios, AirInterdictionZone, AirMission, AirMissionKind, AirPointKind,
    AirPointSource, AirPoints, AirPowerRules, AirStrikeOptions, Airspace, BattleOdds, BattlePlan,
    BattlePlanningRules, BattleReport, Causeway, CityControlChange, CityControlState, CityKind,
    CityOutline, ColumnShift, ColumnShiftReason, CombatObjective, CombatOptions, CombatResult,
    CombatState, CommandLine, CommandZone, CounterattackRoll, Disruption, FormationId, HexId,
    HexsideFeature, LabelKind, LineKind, MapCity, MapDefinition, MapGrid, MapHex, MapHexside,
    MapLabel, MapLine, MapSymbol, MovementMode, MovementModeOptions, MovementOption, NationId,
    OffensiveSupportHq, PendingAdvance, PhaseActor, PhaseDefinition, PhaseExecution, PhaseId,
    PlannedMovement, ReinforcementDefinition, ReinforcementSector, ReserveOption, ReserveState, ScenarioDefinition,
    ScenarioSummary, SideAirPower, SideDefinition, SideId, StepId, StrengthModifier, StrikePlan,
    StrikeResolution, StrikeResult, StrikeTargetHex, StrikeTargetUnit, SupplyStatus, Terrain,
    TrainStatus, UnitDefinition, UnitId, UnitLocation, UnitState, UnitStepDefinition, UnitStrength,
    UnitSupplyState, UnitTraitId, UnitTypeId, WaterArea, ODDS_COLUMNS,
};
pub use setup::{GameSetup, SetupCity, SetupStart, SetupUnit};

#[cfg(test)]
mod tests;
