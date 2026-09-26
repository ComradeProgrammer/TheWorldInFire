//! UI-independent rules engine and domain model for OOA Wargame.
//!
//! The [`model`] module contains scenario and unit data. The crate-level types
//! execute commands, advance the turn state machine, and produce IPC-friendly
//! snapshots and events.

#![deny(missing_docs)]

mod command;
mod engine;
mod error;
mod event;
/// Domain data for sides, phases, scenarios, and units.
pub mod model;
mod phase;
mod state;

pub use command::{CommandOutcome, GameCommand};
pub use error::RuleError;
pub use event::GameEvent;
pub use model::{
    find_scenario, list_scenarios, FormationId, HexId, NationId, PhaseActor, PhaseDefinition,
    PhaseExecution, PhaseId, ReinforcementDefinition, ScenarioDefinition, ScenarioSummary,
    SideDefinition, SideId, StepId, UnitDefinition, UnitId, UnitLocation, UnitState,
    UnitStepDefinition, UnitTraitId, UnitTypeId,
};
pub use state::{GameId, GameSnapshot, GameState, GameStatus, PendingDecision, TurnState};

#[cfg(test)]
mod tests;
