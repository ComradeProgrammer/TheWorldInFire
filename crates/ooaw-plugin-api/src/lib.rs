//! Shared vocabulary of the OOAW kernel and its rules plugins.
//!
//! The kernel (`ooaw-core`) owns game state, dice, the turn sequence, and
//! command routing. Rules plugins, compiled to WebAssembly, decide what every
//! phase and command does. This crate defines what crosses that boundary: the
//! generic game model (identifiers, maps, phases, units, city control) and
//! the plugin contract.
//!
//! Plugins are written in Rust. The contract is the [`sdk::RulesPlugin`] trait
//! together with the types in its signatures, from this module and
//! [`protocol`]. [`export_plugin!`] turns a plugin into a WebAssembly module
//! whose exports, listed in [`abi`], mirror the trait's methods. The crate
//! compiles both natively and for `wasm32`.

#![deny(missing_docs)]

pub mod abi;
mod error;
mod game;
mod ids;
mod map;
mod phase;
pub mod protocol;
pub mod sdk;
mod side;
mod unit;

pub use abi::ABI_VERSION;
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
