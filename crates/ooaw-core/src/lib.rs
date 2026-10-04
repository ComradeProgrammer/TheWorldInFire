//! The OOAW game kernel.
//!
//! The kernel owns everything that is not a game rule: the authoritative
//! mutable state, the seeded dice, the turn sequencer, command routing, and
//! all-or-nothing command execution. Game rules live in rules plugins compiled
//! to WebAssembly and run in a deterministic sandbox. The official NATO rules
//! are such a plugin, bundled into this crate at build time.
//!
//! The kernel/plugin protocol and the shared game model are defined in
//! [`ooaw_plugin_api`], re-exported here as [`api`].

#![deny(missing_docs)]

mod dice;
mod engine;
mod runtime;
mod state;

pub use dice::Dice;
pub use engine::{list_scenarios, CommandOutcome, GameEngine, GameSnapshot, PROTOCOL_VERSION};
pub use ooaw_plugin_api as api;
pub use ooaw_plugin_api::protocol::ScenarioRequest;
pub use ooaw_plugin_api::{GameId, MapDefinition, PhaseDefinition, RuleError, ScenarioSummary};
pub use runtime::{official_plugin, PluginModule};
pub use state::KernelState;
