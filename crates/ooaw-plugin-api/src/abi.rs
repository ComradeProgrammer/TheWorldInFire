//! The WebAssembly functions between the kernel and a rules plugin.
//!
//! A rules plugin is a core WebAssembly module with no WASI imports. Its
//! exports mirror the methods of [`RulesPlugin`](crate::sdk::RulesPlugin) one
//! for one; [`export_plugin!`](crate::export_plugin) generates them from a
//! plugin's implementation of the trait. Its imports are the services the
//! kernel provides during a call.
//!
//! Each function is given twice: its name, and its signature as a
//! function-pointer type. `export_plugin!` and the SDK's imports are checked
//! against these types at compile time; the kernel looks the functions up by
//! these names when it loads a plugin. WebAssembly has no unsigned types:
//! every `u32` is an `i32` and every `u64` an `i64` in the module.
//!
//! # Passing values
//!
//! WebAssembly functions take and return only numbers. Any other argument or
//! result lives in the plugin's linear memory and is passed as a pointer and a
//! byte length:
//!
//! - Text (a filter name, a log message) is UTF-8.
//! - Every other value is MessagePack, written by [`encode`] and read by
//!   [`decode`] on both sides. Several arguments travel as one tuple.
//! - A function that returns a value returns `(ptr << 32) | len`. The buffer
//!   belongs to the kernel, which frees it with `ooaw_free`.
//! - A fallible result is a `Result<T, RuleError>`, so a rule rejection
//!   crosses the boundary as data rather than as a trap.
//!
//! The kernel never re-enters a plugin that is already handling a call.

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::RuleError;

/// Version of the functions in this module and of how they pass values. The
/// kernel rejects a plugin built against another version.
pub const ABI_VERSION: u32 = 3;

/// Encodes a value passed across the boundary.
///
/// # Errors
///
/// Returns `pluginProtocol` when the value cannot be encoded.
pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, RuleError> {
    rmp_serde::to_vec_named(value)
        .map_err(|error| RuleError::protocol(format!("Unencodable value: {error}")))
}

/// Decodes a value passed across the boundary.
///
/// # Errors
///
/// Returns `pluginProtocol` when the bytes are not an encoded `T`.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, RuleError> {
    rmp_serde::from_slice(bytes)
        .map_err(|error| RuleError::protocol(format!("Malformed value: {error}")))
}

/// Functions the plugin exports and the kernel calls.
///
/// Before each call the kernel calls [`SET_TURN`](exports::SET_TURN), and
/// [`SYNC`](exports::SYNC) when the plugin's mirror is stale. After a
/// successful call that may change state it calls
/// [`TAKE_CHANGES`](exports::TAKE_CHANGES).
pub mod exports {
    /// The plugin's linear memory, which holds every argument and result.
    pub const MEMORY: &str = "memory";

    /// Returns [`ABI_VERSION`](crate::abi::ABI_VERSION).
    pub const VERSION: &str = "ooaw_abi_version";
    /// Signature of [`VERSION`].
    pub type Version = unsafe extern "C" fn() -> u32;

    /// Allocates `len` bytes for the kernel to write an argument into, and
    /// returns their address. The callee takes ownership of the buffer.
    pub const ALLOC: &str = "ooaw_alloc";
    /// Signature of [`ALLOC`].
    pub type Alloc = unsafe extern "C" fn(len: u32) -> u32;

    /// Frees a result buffer.
    pub const FREE: &str = "ooaw_free";
    /// Signature of [`FREE`].
    pub type Free = unsafe extern "C" fn(ptr: u32, len: u32);

    /// `RulesPlugin::manifest`. Returns a `PluginManifest`.
    pub const MANIFEST: &str = "ooaw_manifest";
    /// Signature of [`MANIFEST`].
    pub type Manifest = unsafe extern "C" fn() -> u64;

    /// `RulesPlugin::set_turn`. `completed` is 1 once the game has ended,
    /// otherwise 0.
    pub const SET_TURN: &str = "ooaw_set_turn";
    /// Signature of [`SET_TURN`].
    pub type SetTurn = unsafe extern "C" fn(game_turn: u32, step_index: u32, completed: u32);

    /// `RulesPlugin::sync`. Takes a `GameState`; returns `Result<()>`.
    pub const SYNC: &str = "ooaw_sync";
    /// `RulesPlugin::take_changes`. Returns `Result<Vec<Change>>`.
    pub const TAKE_CHANGES: &str = "ooaw_take_changes";
    /// Signature of [`TAKE_CHANGES`].
    pub type TakeChanges = unsafe extern "C" fn() -> u64;

    /// `RulesPlugin::create_game`. Takes `(GameId, ScenarioRequest)`; returns
    /// `Result<ScenarioSetup>`.
    pub const CREATE_GAME: &str = "ooaw_create_game";
    /// `RulesPlugin::attach`. Takes an `Attachment`; returns `Result<()>`.
    pub const ATTACH: &str = "ooaw_attach";
    /// `RulesPlugin::apply_setup`. Takes the setup document; returns
    /// `Result<Vec<event>>`.
    pub const APPLY_SETUP: &str = "ooaw_apply_setup";
    /// `RulesPlugin::phase_started`. Takes a `PhaseDefinition`; returns
    /// `Result<Vec<event>>`.
    pub const PHASE_STARTED: &str = "ooaw_phase_started";
    /// `RulesPlugin::phase_ending`. Takes a `PhaseDefinition`; returns
    /// `Result<Vec<event>>`.
    pub const PHASE_ENDING: &str = "ooaw_phase_ending";
    /// `RulesPlugin::command`. Takes the command; returns `Result<Vec<event>>`.
    pub const COMMAND: &str = "ooaw_command";
    /// `RulesPlugin::query`. Takes `(name, input)`; returns `Result<value>`.
    pub const QUERY: &str = "ooaw_query";
    /// `RulesPlugin::filter`. Takes `(name, input, value)`; returns
    /// `Result<value>`.
    pub const FILTER: &str = "ooaw_filter";

    /// Signature of every export that takes one encoded argument and returns
    /// an encoded result: [`SYNC`], [`CREATE_GAME`], [`ATTACH`],
    /// [`APPLY_SETUP`], [`PHASE_STARTED`], [`PHASE_ENDING`], [`COMMAND`],
    /// [`QUERY`], and [`FILTER`].
    pub type Method = unsafe extern "C" fn(ptr: u32, len: u32) -> u64;
}

/// Functions the kernel provides and the plugin may import.
pub mod imports {
    /// Import module of every function below.
    pub const MODULE: &str = "ooaw";

    /// Returns the next value, `1..=sides`, of the game's seeded dice. Traps
    /// during queries and filters, which must not consume dice.
    pub const ROLL: &str = "roll";
    /// Signature of [`ROLL`].
    pub type Roll = unsafe extern "C" fn(sides: u32) -> u32;

    /// Returns 1 when another plugin takes part in the filter named by the
    /// UTF-8 text at `ptr`, otherwise 0.
    pub const FILTER_ACTIVE: &str = "filter_active";
    /// Signature of [`FILTER_ACTIVE`].
    pub type FilterActive = unsafe extern "C" fn(ptr: u32, len: u32) -> u32;

    /// Passes a value through every other plugin taking part in a filter.
    /// Takes `(name, input, value, changes)`, where `changes` are the caller's
    /// changes not yet reported, which the kernel applies first. Returns the
    /// byte length of a `Result<value>` that [`HOST_READ`] then copies.
    pub const FILTER: &str = "filter";
    /// Signature of [`FILTER`].
    pub type Filter = unsafe extern "C" fn(ptr: u32, len: u32) -> u32;

    /// Copies the result of the last [`FILTER`] to `ptr`, which must have
    /// room for its length.
    pub const HOST_READ: &str = "host_read";
    /// Signature of [`HOST_READ`].
    pub type HostRead = unsafe extern "C" fn(ptr: u32);

    /// Writes the UTF-8 text at `ptr` to the kernel's log.
    pub const LOG: &str = "log";
    /// Signature of [`LOG`].
    pub type Log = unsafe extern "C" fn(ptr: u32, len: u32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Change;
    use crate::serde_json::{json, Value};
    use crate::{GameState, Unit};

    // MessagePack must carry what crosses the boundary: internally tagged
    // enums, flattened unit markers, open-ended values, tuples, and results.
    #[test]
    fn values_round_trip() {
        let unit: Unit = serde_json::from_value(json!({
            "id": "a", "name": "a", "sideId": "nato", "nationId": "westGermany",
            "unitTypeId": "armoredBrigade", "formationId": null, "traits": [],
            "steps": [{ "attack": 3, "defense": 2.5 }], "strengthStepIndex": 0,
            "location": { "type": "hex", "hexId": "0101" },
            "supply": "supplied", "disruption": { "level": -1, "flags": [true, null] },
        }))
        .unwrap();
        let mut state = GameState::default();
        state.units.push(unit.clone());
        state
            .rules
            .insert("battlePlan".to_owned(), json!({ "battles": [] }));
        assert_eq!(
            decode::<GameState>(&encode(&state).unwrap()).unwrap(),
            state
        );
        // A plugin reads the state as an open-ended value.
        let mirror: Value = decode(&encode(&state).unwrap()).unwrap();
        assert_eq!(mirror["units"][0]["disruption"]["level"], json!(-1));

        let args = (
            "nato.combat.columnShifts".to_owned(),
            json!({ "battle": 1 }),
            json!([-1]),
        );
        assert_eq!(
            decode::<(String, Value, Value)>(&encode(&args).unwrap()).unwrap(),
            args
        );

        let changes: Result<Vec<Change>, RuleError> = Ok(vec![
            Change::PutUnit { unit },
            Change::SetPendingDecision { decision: None },
        ]);
        assert_eq!(
            decode::<Result<Vec<Change>, RuleError>>(&encode(&changes).unwrap()).unwrap(),
            changes
        );
        let rejected: Result<Vec<Value>, RuleError> = Err(RuleError::new("illegalMove", "Blocked"));
        assert_eq!(
            decode::<Result<Vec<Value>, RuleError>>(&encode(&rejected).unwrap()).unwrap(),
            rejected
        );
    }
}
