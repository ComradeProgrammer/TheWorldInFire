//! Shared vocabulary of the OOAW kernel and its rules plugins.
//!
//! The kernel (`ooaw-core`) owns game state, dice, the turn sequence, and
//! command routing. Rules plugins, compiled to WebAssembly, decide what every
//! phase and command does. This crate defines what crosses that boundary: the
//! generic game model (identifiers, maps, phases, units, city control) and the
//! [`protocol`] messages. It compiles both natively and for `wasm32`.

#![deny(missing_docs)]

mod error;
mod game;
mod ids;
mod map;
mod phase;
pub mod protocol;
mod side;
mod unit;

pub use error::RuleError;
pub use game::{
    CityControlState, GameState, GameStatus, PendingDecision, ScenarioSummary, TurnPosition,
    TurnState,
};
pub use ids::{FormationId, GameId, HexId, NationId, UnitId, UnitTraitId, UnitTypeId};
pub use map::{
    Causeway, CityKind, CityOutline, CommandLine, CommandZone, HexsideFeature, LabelKind, LineKind,
    MapCity, MapDefinition, MapGrid, MapHex, MapHexside, MapLabel, MapLine, MapSymbol,
    ReinforcementSector, Terrain, WaterArea,
};
pub use phase::{PhaseActor, PhaseDefinition, PhaseExecution, PhaseId, StepId};
pub use side::{SideDefinition, SideId};
pub use unit::{Unit, UnitLocation, KERNEL_UNIT_FIELDS};

pub use serde_json;
