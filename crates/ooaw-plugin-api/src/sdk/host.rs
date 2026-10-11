//! Services the kernel provides to a running plugin.
//!
//! These functions call the kernel's imports, listed in
//! [`crate::abi::imports`], when compiled for `wasm32`. In a native build
//! there is no kernel: [`roll`] panics, [`filter_active`] is false, [`filter`]
//! returns the value unchanged, and [`log`] prints to standard error.

use crate::protocol::Change;
use crate::serde_json::Value;
use crate::RuleError;

/// Whether another plugin takes part in the filter during the current call.
/// The single-plugin kernel always returns false.
pub fn filter_active(name: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        // SAFETY: the kernel only reads `name` during the call.
        unsafe { imports::filter_active(name.as_ptr() as usize as u32, name.len() as u32) != 0 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = name;
        false
    }
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

/// Compatibility wrapper for the former multi-plugin filter mechanism.
///
/// In the single-plugin kernel, `value` is returned unchanged. If supplied,
/// `changes` are applied first. Callers normally check [`filter_active`] and
/// skip the call because it always returns false.
pub fn filter(
    name: &str,
    input: Value,
    value: Value,
    changes: Vec<Change>,
) -> Result<Value, RuleError> {
    if changes.is_empty() && !filter_active(name) {
        return Ok(value);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let args = crate::abi::encode(&(name, input, value, changes))?;
        // SAFETY: the kernel reads `args` during the call, then writes exactly
        // `len` bytes into the buffer passed to `host_read`.
        let result = unsafe {
            let len = imports::filter(args.as_ptr() as usize as u32, args.len() as u32) as usize;
            let mut buffer = vec![0u8; len];
            imports::host_read(buffer.as_mut_ptr() as usize as u32);
            buffer
        };
        crate::abi::decode::<Result<Value, RuleError>>(&result)?
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = input;
        Ok(value)
    }
}

/// Writes a diagnostic message to the kernel's log.
pub fn log(message: &str) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: the kernel only reads `message` during the call.
    unsafe {
        imports::log(message.as_ptr() as usize as u32, message.len() as u32)
    }
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{message}");
}

/// The kernel's imports, as listed in [`crate::abi::imports`].
#[cfg(target_arch = "wasm32")]
mod imports {
    use crate::abi::imports as abi;

    // `wasm_import_module` must be a literal; check the signatures instead.
    #[link(wasm_import_module = "ooaw")]
    extern "C" {
        pub fn roll(sides: u32) -> u32;
        pub fn filter_active(ptr: u32, len: u32) -> u32;
        pub fn filter(ptr: u32, len: u32) -> u32;
        pub fn host_read(ptr: u32);
        pub fn log(ptr: u32, len: u32);
    }

    const _: abi::Roll = roll;
    const _: abi::FilterActive = filter_active;
    const _: abi::Filter = filter;
    const _: abi::HostRead = host_read;
    const _: abi::Log = log;
}
