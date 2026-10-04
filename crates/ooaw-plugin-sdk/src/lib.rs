//! Guest-side support for writing OOAW rules plugins in Rust.
//!
//! Implement [`RulesPlugin`] for a type and invoke [`export_plugin!`] once in
//! the crate root. Build the crate as a `cdylib` for `wasm32-unknown-unknown`;
//! the macro generates the exports described in [`api::protocol`].
//!
//! A plugin keeps a mirror of the kernel's mutable state. The kernel sends the
//! complete state only when the mirror is stale, so a plugin changes its mirror
//! directly and reports the difference from [`RulesPlugin::take_changes`]. The
//! [`mirror`] helpers compute those differences.

#![deny(missing_docs)]

pub mod host;
pub mod mirror;

#[doc(hidden)]
pub mod __private;

pub use ooaw_plugin_api as api;

use api::protocol::{Change, PluginManifest, ScenarioRequest, ScenarioSetup};
use api::serde_json::Value;
use api::{GameId, MapDefinition, PhaseDefinition, RuleError, ScenarioSummary, TurnPosition};

/// Fields of a [`api::protocol::PluginCall::Attach`] call.
#[derive(Clone, Debug)]
pub struct Attachment {
    /// Game identifier.
    pub game_id: GameId,
    /// Scenario the game was created from.
    pub scenario: ScenarioRequest,
    /// Client-facing scenario metadata.
    pub summary: ScenarioSummary,
    /// Turn sequence of the game.
    pub turn_sequence: Vec<PhaseDefinition>,
    /// The map, present when the plugin does not own the scenario.
    pub map: Option<Box<MapDefinition>>,
}

/// Rules implemented by one plugin.
///
/// One value of the implementing type serves one game. Mutating handlers
/// (`apply_setup`, `phase_started`, `phase_ending`, `command`) change the
/// mirror and return domain events as JSON objects with a `type` tag;
/// [`Self::take_changes`] then reports what changed. Read-only handlers
/// (`query`, `filter`) must leave the mirror unchanged and roll no dice.
pub trait RulesPlugin: Default + 'static {
    /// Describes what the plugin provides.
    fn manifest(&self) -> PluginManifest;

    /// Builds a scenario this plugin owns and returns its content and opening
    /// state; the plugin then serves the new game.
    fn create_game(
        &mut self,
        game_id: GameId,
        scenario: ScenarioRequest,
    ) -> Result<ScenarioSetup, RuleError> {
        let _ = game_id;
        Err(RuleError::new(
            "scenarioNotFound",
            format!("This plugin provides no scenario {}", scenario.id),
        ))
    }

    /// Prepares the plugin to serve an existing game.
    fn attach(&mut self, attachment: Attachment) -> Result<(), RuleError>;

    /// Records the current turn position, sent with every call.
    fn set_turn(&mut self, turn: TurnPosition);

    /// Replaces the mirror with the complete state (a serialized
    /// [`api::GameState`]) and makes it the baseline for future changes.
    fn sync(&mut self, state: Value) -> Result<(), RuleError>;

    /// Returns the changes made to the mirror since the last sync or call and
    /// makes the current mirror the new baseline.
    ///
    /// Takes `&self` so a read-only handler in the middle of a command can
    /// send pending changes with a [`host::filter`] call; keep the baseline in
    /// a `RefCell`.
    fn take_changes(&self) -> Result<Vec<Change>, RuleError>;

    /// Lays out a custom starting situation in the plugin's own setup format.
    fn apply_setup(&mut self, setup: Value) -> Result<Vec<Value>, RuleError> {
        let _ = setup;
        Err(RuleError::new(
            "setupUnsupported",
            "This plugin does not accept custom setups",
        ))
    }

    /// Runs the automatic work of a phase that has just started.
    fn phase_started(&mut self, phase: &PhaseDefinition) -> Result<Vec<Value>, RuleError> {
        let _ = phase;
        Ok(Vec::new())
    }

    /// Cleans up a phase that is ending, or rejects to keep it current.
    fn phase_ending(&mut self, phase: &PhaseDefinition) -> Result<Vec<Value>, RuleError> {
        let _ = phase;
        Ok(Vec::new())
    }

    /// Executes a player command declared in the manifest.
    fn command(&mut self, command: Value) -> Result<Vec<Value>, RuleError> {
        let _ = command;
        Err(RuleError::new(
            "unknownCommand",
            "This plugin executes no commands",
        ))
    }

    /// Answers a read-only query declared in the manifest.
    fn query(&self, name: &str, input: Value) -> Result<Value, RuleError> {
        let _ = input;
        Err(RuleError::new(
            "unknownQuery",
            format!("This plugin answers no query {name}"),
        ))
    }

    /// Takes part in a filter declared in the manifest by adjusting `value`.
    fn filter(&self, name: &str, input: Value, value: Value) -> Result<Value, RuleError> {
        let _ = (name, input);
        Ok(value)
    }
}

/// Generates the WebAssembly exports for a [`RulesPlugin`] type.
///
/// Invoke once, in the plugin crate's root module. The exports exist only when
/// compiling for `wasm32`, so the crate can still be used natively as a library.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        #[cfg(target_arch = "wasm32")]
        const _: () = {
            ::std::thread_local! {
                static PLUGIN: ::core::cell::RefCell<::core::option::Option<$plugin>> =
                    const { ::core::cell::RefCell::new(::core::option::Option::None) };
            }

            #[no_mangle]
            pub extern "C" fn ooaw_abi_version() -> u32 {
                $crate::api::protocol::ABI_VERSION
            }

            #[no_mangle]
            pub extern "C" fn ooaw_alloc(len: u32) -> u32 {
                $crate::__private::alloc(len)
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_free(ptr: u32, len: u32) {
                $crate::__private::free(ptr, len)
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_dispatch(ptr: u32, len: u32) -> u64 {
                let request = $crate::__private::take(ptr, len);
                let response =
                    PLUGIN.with(|slot| $crate::__private::dispatch::<$plugin>(slot, &request));
                $crate::__private::leak(response)
            }
        };
    };
}
