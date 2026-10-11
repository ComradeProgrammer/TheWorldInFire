//! Writing OOAW rules plugins, which are written in Rust.
//!
//! [`RulesPlugin`] is the contract between the kernel and a plugin: the kernel
//! drives a game by calling its methods. Implement it for a type and invoke
//! [`export_plugin!`](crate::export_plugin) once in the crate root, then build
//! the crate as a `cdylib` for `wasm32-unknown-unknown`. The macro generates one
//! WebAssembly export per trait method, as listed in [`crate::abi`]; each
//! decodes its arguments, calls the method, and encodes the result.
//!
//! A plugin keeps a mirror of the kernel's mutable state. The kernel sends the
//! complete state only when the mirror is stale, so a plugin changes its mirror
//! directly and reports the difference from [`RulesPlugin::take_changes`]. The
//! [`mirror`] helpers compute those differences.

pub mod host;
pub mod mirror;

#[doc(hidden)]
pub mod __private;

pub use crate::protocol::Attachment;
use crate::protocol::{Change, PluginManifest, ScenarioRequest, ScenarioSetup};
use crate::serde_json::Value;
use crate::{GameCommand, GameId, PhaseDefinition, RuleError, TurnPosition};

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
    /// [`crate::GameState`]) and makes it the baseline for future changes.
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

    /// Executes one command from the shared, fixed command vocabulary.
    fn command(&mut self, command: GameCommand) -> Result<Vec<Value>, RuleError> {
        let _ = command;
        Err(RuleError::new(
            "unsupportedCommand",
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
/// Their names and signatures are those in [`crate::abi::exports`]; the build
/// fails if a signature drifts from it.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        #[cfg(target_arch = "wasm32")]
        const _: () = {
            use $crate::abi::exports as abi;
            use $crate::sdk::__private as glue;
            use $crate::sdk::RulesPlugin;
            use $crate::serde_json::Value;

            ::std::thread_local! {
                static PLUGIN: glue::Slot<$plugin> = const { glue::Slot::new(None) };
            }

            #[no_mangle]
            pub extern "C" fn ooaw_abi_version() -> u32 {
                $crate::ABI_VERSION
            }

            #[no_mangle]
            pub extern "C" fn ooaw_alloc(len: u32) -> u32 {
                glue::alloc(len)
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_free(ptr: u32, len: u32) {
                glue::free(ptr, len)
            }

            #[no_mangle]
            pub extern "C" fn ooaw_manifest() -> u64 {
                glue::with_plugin(&PLUGIN, |plugin: &mut $plugin| {
                    glue::reply(&plugin.manifest())
                })
            }

            #[no_mangle]
            pub extern "C" fn ooaw_set_turn(game_turn: u32, step_index: u32, completed: u32) {
                let turn = glue::turn(game_turn, step_index, completed);
                glue::with_plugin(&PLUGIN, |plugin: &mut $plugin| plugin.set_turn(turn))
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_sync(ptr: u32, len: u32) -> u64 {
                glue::call(&PLUGIN, ptr, len, |plugin: &mut $plugin, state: Value| {
                    plugin.sync(state)
                })
            }

            #[no_mangle]
            pub extern "C" fn ooaw_take_changes() -> u64 {
                glue::with_plugin(&PLUGIN, |plugin: &mut $plugin| {
                    glue::reply(&plugin.take_changes())
                })
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_create_game(ptr: u32, len: u32) -> u64 {
                type Args = ($crate::GameId, $crate::protocol::ScenarioRequest);
                glue::call(&PLUGIN, ptr, len, |plugin: &mut $plugin, args: Args| {
                    plugin.create_game(args.0, args.1)
                })
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_attach(ptr: u32, len: u32) -> u64 {
                glue::call(
                    &PLUGIN,
                    ptr,
                    len,
                    |plugin: &mut $plugin, attachment: $crate::protocol::Attachment| {
                        plugin.attach(attachment)
                    },
                )
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_apply_setup(ptr: u32, len: u32) -> u64 {
                glue::call(&PLUGIN, ptr, len, |plugin: &mut $plugin, setup: Value| {
                    plugin.apply_setup(setup)
                })
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_phase_started(ptr: u32, len: u32) -> u64 {
                glue::call(
                    &PLUGIN,
                    ptr,
                    len,
                    |plugin: &mut $plugin, phase: $crate::PhaseDefinition| {
                        plugin.phase_started(&phase)
                    },
                )
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_phase_ending(ptr: u32, len: u32) -> u64 {
                glue::call(
                    &PLUGIN,
                    ptr,
                    len,
                    |plugin: &mut $plugin, phase: $crate::PhaseDefinition| {
                        plugin.phase_ending(&phase)
                    },
                )
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_command(ptr: u32, len: u32) -> u64 {
                glue::call(
                    &PLUGIN,
                    ptr,
                    len,
                    |plugin: &mut $plugin, command: $crate::GameCommand| plugin.command(command),
                )
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_query(ptr: u32, len: u32) -> u64 {
                glue::call(
                    &PLUGIN,
                    ptr,
                    len,
                    |plugin: &mut $plugin, (name, input): (String, Value)| {
                        plugin.query(&name, input)
                    },
                )
            }

            #[no_mangle]
            pub unsafe extern "C" fn ooaw_filter(ptr: u32, len: u32) -> u64 {
                glue::call(
                    &PLUGIN,
                    ptr,
                    len,
                    |plugin: &mut $plugin, (name, input, value): (String, Value, Value)| {
                        plugin.filter(&name, input, value)
                    },
                )
            }

            // `#[no_mangle]` names must be literal; check the signatures instead.
            const _: abi::Version = ooaw_abi_version;
            const _: abi::Alloc = ooaw_alloc;
            const _: abi::Free = ooaw_free;
            const _: abi::Manifest = ooaw_manifest;
            const _: abi::SetTurn = ooaw_set_turn;
            const _: abi::Method = ooaw_sync;
            const _: abi::TakeChanges = ooaw_take_changes;
            const _: abi::Method = ooaw_create_game;
            const _: abi::Method = ooaw_attach;
            const _: abi::Method = ooaw_apply_setup;
            const _: abi::Method = ooaw_phase_started;
            const _: abi::Method = ooaw_phase_ending;
            const _: abi::Method = ooaw_command;
            const _: abi::Method = ooaw_query;
            const _: abi::Method = ooaw_filter;
        };
    };
}
