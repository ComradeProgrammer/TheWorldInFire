//! Official OOAW rules plugin: NATO: The Cold War Goes Hot.
//!
//! Built for `wasm32-unknown-unknown`, this crate is the WebAssembly module
//! the kernel (`ooaw-core`) bundles and loads for every NATO scenario. Built
//! natively, it is a library of the rules' typed model (units, commands,
//! events, battle reports) for tests and for plugins that extend these rules.
//!
//! [`model`] holds scenario and unit data. The other modules implement the
//! phases and commands against a mirror of the kernel's state.

#![deny(missing_docs)]

mod airspace;
mod cities;
mod combat;
mod command;
mod dice;
mod error;
mod event;
pub mod filters;
/// Domain data for sides, phases, scenarios, and units.
pub mod model;
mod movement;
mod phase;
mod planning;
mod plugin;
mod reserve;
mod rules;
mod setup;
mod strikes;
mod supply;

pub use combat::parse_result;
pub use command::GameCommand;
pub use error::RuleError;
pub use event::{GameEvent, UnitSupplyCheck};
pub use model::*;
pub use plugin::{NatoPlugin, PLUGIN_ID};
pub use rules::{Rules, RulesState};
pub use setup::{GameSetup, SetupCity, SetupStart, SetupUnit};
pub use strikes::strike_table;

ooaw_plugin_sdk::export_plugin!(NatoPlugin);
