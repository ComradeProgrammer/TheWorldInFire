//! Implementation details of [`crate::export_plugin!`].

use std::cell::RefCell;

use crate::api::protocol::{DispatchRequest, DispatchResponse, PluginCall};
use crate::api::serde_json::{self, Value};
use crate::api::RuleError;
use crate::{host, Attachment, RulesPlugin};

/// Allocates an uninitialised buffer of `len` bytes and leaks it to the kernel.
pub fn alloc(len: u32) -> u32 {
    let mut buffer = Vec::<u8>::with_capacity(len as usize);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr as usize as u32
}

/// Frees a buffer created by [`alloc`] or [`leak`].
///
/// # Safety
///
/// `ptr` and `len` must describe a buffer from [`alloc`] or [`leak`] that has
/// not been freed.
pub unsafe fn free(ptr: u32, len: u32) {
    drop(Vec::from_raw_parts(
        ptr as usize as *mut u8,
        0,
        len as usize,
    ));
}

/// Takes ownership of a buffer from [`alloc`] that the kernel has filled.
///
/// # Safety
///
/// `ptr` and `len` must describe a buffer from [`alloc`] whose `len` bytes
/// have all been written.
pub unsafe fn take(ptr: u32, len: u32) -> Vec<u8> {
    Vec::from_raw_parts(ptr as usize as *mut u8, len as usize, len as usize)
}

/// Leaks a response buffer, returning `(ptr << 32) | len`.
pub fn leak(bytes: Vec<u8>) -> u64 {
    let bytes = bytes.into_boxed_slice();
    let len = bytes.len() as u64;
    let ptr = Box::into_raw(bytes) as *mut u8 as usize as u64;
    (ptr << 32) | len
}

/// Decodes one request, runs it against the plugin in `slot`, and encodes the response.
pub fn dispatch<P: RulesPlugin>(slot: &RefCell<Option<P>>, request: &[u8]) -> Vec<u8> {
    let response = match serde_json::from_slice::<DispatchRequest<Value>>(request) {
        Ok(request) => handle(slot, request),
        Err(error) => failure(RuleError::protocol(format!("Malformed request: {error}"))),
    };
    serde_json::to_vec(&response).unwrap_or_else(|error| {
        serde_json::to_vec(&failure(RuleError::protocol(format!(
            "Unserializable response: {error}"
        ))))
        .expect("an error response always serializes")
    })
}

fn failure(error: RuleError) -> DispatchResponse {
    DispatchResponse {
        result: Err(error),
        changes: Vec::new(),
        events: Vec::new(),
    }
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, RuleError> {
    serde_json::to_value(value).map_err(|error| RuleError::protocol(error.to_string()))
}

fn handle<P: RulesPlugin>(
    slot: &RefCell<Option<P>>,
    request: DispatchRequest<Value>,
) -> DispatchResponse {
    let mut guard = slot.borrow_mut();
    let plugin = guard.get_or_insert_with(P::default);
    host::set_active_filters(request.active_filters);
    if let Some(turn) = request.turn {
        plugin.set_turn(turn);
    }
    if let Some(state) = request.sync {
        if let Err(error) = plugin.sync(state) {
            return failure(error);
        }
    }

    let outcome: Result<(Value, Vec<Value>), RuleError> = match request.call {
        PluginCall::Manifest => to_value(plugin.manifest()).map(|value| (value, Vec::new())),
        PluginCall::CreateGame { game_id, scenario } => plugin
            .create_game(game_id, scenario)
            .and_then(to_value)
            .map(|value| (value, Vec::new())),
        PluginCall::Attach {
            game_id,
            scenario,
            summary,
            turn_sequence,
            map,
        } => plugin
            .attach(Attachment {
                game_id,
                scenario,
                summary,
                turn_sequence,
                map,
            })
            .map(|()| (Value::Null, Vec::new())),
        PluginCall::ApplySetup { setup } => plugin
            .apply_setup(setup)
            .map(|events| (Value::Null, events)),
        PluginCall::PhaseStarted { phase } => plugin
            .phase_started(&phase)
            .map(|events| (Value::Null, events)),
        PluginCall::PhaseEnding { phase } => plugin
            .phase_ending(&phase)
            .map(|events| (Value::Null, events)),
        PluginCall::Command { command } => {
            plugin.command(command).map(|events| (Value::Null, events))
        }
        PluginCall::Query { name, input } => {
            plugin.query(&name, input).map(|value| (value, Vec::new()))
        }
        PluginCall::Filter { name, input, value } => plugin
            .filter(&name, input, value)
            .map(|value| (value, Vec::new())),
    };

    match outcome {
        // The kernel restores its state after a failed call and resynchronises
        // this plugin, so changes made before the failure need not be reported.
        Err(error) => failure(error),
        Ok((value, events)) => match plugin.take_changes() {
            Ok(changes) => DispatchResponse {
                result: Ok(value),
                changes,
                events,
            },
            Err(error) => failure(error),
        },
    }
}
