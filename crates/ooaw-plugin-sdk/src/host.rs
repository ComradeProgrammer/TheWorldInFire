//! Services the kernel provides to a running plugin.
//!
//! These functions call the kernel's imports when compiled for `wasm32`. In a
//! native build there is no kernel: [`roll`] panics, [`filter`] returns the
//! value unchanged, and [`log`] prints to standard error.

use std::cell::RefCell;

use crate::api::protocol::{Change, HostRequest, HostResponse};
use crate::api::serde_json::Value;
use crate::api::RuleError;

thread_local! {
    static ACTIVE_FILTERS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn set_active_filters(filters: Vec<String>) {
    ACTIVE_FILTERS.with(|active| *active.borrow_mut() = filters);
}

/// Whether another plugin takes part in the filter during the current call.
/// When it does not, [`filter`] would return its value unchanged.
pub fn filter_active(name: &str) -> bool {
    ACTIVE_FILTERS.with(|active| active.borrow().iter().any(|filter| filter == name))
}

/// Rolls the game's seeded dice, returning a value from 1 to `sides`.
///
/// # Panics
///
/// Traps the plugin when called from a query or filter, which must not
/// consume dice, and panics in a native build.
pub fn roll(sides: u32) -> u8 {
    #[cfg(target_arch = "wasm32")]
    {
        // SAFETY: a plain import call with integer arguments.
        unsafe { imports::roll(sides) as u8 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = sides;
        panic!("dice are only available to a plugin running in the kernel")
    }
}

/// Passes `value` through every other plugin taking part in filter `name`.
///
/// `changes` are the caller's changes not yet reported in this call; the
/// kernel applies them first so the other plugins see current state. Callers
/// normally check [`filter_active`] first and skip both the call and the
/// change collection when it returns false.
pub fn filter(
    name: &str,
    input: Value,
    value: Value,
    changes: Vec<Change>,
) -> Result<Value, RuleError> {
    if !filter_active(name) && changes.is_empty() {
        return Ok(value);
    }
    request(&HostRequest::Filter {
        name: name.to_owned(),
        input,
        value,
        changes,
    })
}

/// Writes a diagnostic message to the kernel's log.
pub fn log(message: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = request(&HostRequest::Log {
            message: message.to_owned(),
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{message}");
}

#[cfg(target_arch = "wasm32")]
fn request(request: &HostRequest) -> HostResponse {
    let bytes = crate::api::serde_json::to_vec(request)
        .map_err(|error| RuleError::protocol(error.to_string()))?;
    // SAFETY: the kernel reads `bytes` during the call, then writes exactly
    // `len` bytes into the buffer passed to `host_read`.
    let response = unsafe {
        let len = imports::host_call(bytes.as_ptr() as usize as u32, bytes.len() as u32) as usize;
        let mut buffer = vec![0u8; len];
        imports::host_read(buffer.as_mut_ptr() as usize as u32);
        buffer
    };
    crate::api::serde_json::from_slice::<HostResponse>(&response)
        .map_err(|error| RuleError::protocol(format!("Malformed kernel response: {error}")))?
}

#[cfg(not(target_arch = "wasm32"))]
fn request(request: &HostRequest) -> HostResponse {
    match request {
        HostRequest::Filter { value, .. } => Ok(value.clone()),
        HostRequest::Log { message } => {
            eprintln!("{message}");
            Ok(Value::Null)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imports {
    #[link(wasm_import_module = "ooaw")]
    extern "C" {
        pub fn roll(sides: u32) -> u32;
        pub fn host_call(ptr: u32, len: u32) -> u32;
        pub fn host_read(ptr: u32);
    }
}
