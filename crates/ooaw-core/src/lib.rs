//! UI-independent rules engine and domain model for OOA Wargame.
//!
//! The [`model`] module contains scenario and unit data. The crate-level types
//! execute commands, advance the turn state machine, and produce IPC-friendly
//! snapshots and events.

#![deny(missing_docs)]

mod airspace;
mod cities;
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
mod state;
mod strikes;

pub use command::{CommandOutcome, GameCommand};
pub use error::RuleError;
pub use event::{GameEvent, UnitSupplyCheck};
pub use model::{
    find_scenario, list_scenarios, AirInterdictionZone, AirMission, AirMissionKind, AirPointKind,
    AirPointSource, AirPoints, AirPowerRules, AirStrikeOptions, Airspace, BattlePlan,
    BattlePlanningRules, Causeway, CityControlChange, CityControlState, CityKind, CityOutline,
    CommandLine, CommandZone, Disruption, FormationId, HexId, HexsideFeature, LabelKind, LineKind,
    MapCity, MapDefinition, MapGrid, MapHex, MapHexside, MapLabel, MapLine, MapSymbol,
    MovementMode, MovementModeOptions, MovementOption, NationId, PhaseActor, PhaseDefinition,
    PhaseExecution, PhaseId, PlannedMovement, ReinforcementDefinition, ScenarioDefinition,
    ScenarioSummary, SideAirPower, SideDefinition, SideId, StepId, StrikePlan, StrikeResolution,
    StrikeResult, StrikeTargetHex, StrikeTargetUnit, SupplyStatus, Terrain, TrainStatus,
    UnitDefinition, UnitId, UnitLocation, UnitState, UnitStepDefinition, UnitSupplyState,
    UnitTraitId, UnitTypeId, WaterArea,
};
pub use state::{GameId, GameSnapshot, GameState, GameStatus, PendingDecision, TurnState};

#[cfg(test)]
mod tests;
