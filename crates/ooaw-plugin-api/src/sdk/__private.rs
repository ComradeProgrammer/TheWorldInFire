//! Implementation details of [`crate::export_plugin!`].

use std::cell::RefCell;
use std::thread::LocalKey;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::sdk::RulesPlugin;
use crate::{abi, GameStatus, RuleError, TurnPosition};

/// The plugin value serving the game, created on first use.
pub type Slot<P> = RefCell<Option<P>>;

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

/// Runs `f` against the plugin in `slot`, creating it first if needed.
pub fn with_plugin<P: RulesPlugin, R>(
    slot: &'static LocalKey<Slot<P>>,
    f: impl FnOnce(&mut P) -> R,
) -> R {
    slot.with(|cell| f(cell.borrow_mut().get_or_insert_with(P::default)))
}

/// Encodes a result and leaks it to the kernel, returning `(ptr << 32) | len`.
/// A value that cannot be encoded is replaced by the encoding error.
pub fn reply<T: Serialize>(value: &T) -> u64 {
    let bytes = abi::encode(value).unwrap_or_else(|error| {
        abi::encode(&Err::<(), _>(error)).expect("an error always encodes")
    });
    leak(bytes)
}

/// Decodes the argument the kernel wrote at `ptr`, calls `method` with it,
/// and replies with the result.
///
/// # Safety
///
/// `ptr` and `len` must describe a buffer from [`alloc`] whose `len` bytes
/// have all been written.
pub unsafe fn call<P: RulesPlugin, A: DeserializeOwned, R: Serialize>(
    slot: &'static LocalKey<Slot<P>>,
    ptr: u32,
    len: u32,
    method: impl FnOnce(&mut P, A) -> Result<R, RuleError>,
) -> u64 {
    let bytes = take(ptr, len);
    let result =
        abi::decode::<A>(&bytes).and_then(|args| with_plugin(slot, |plugin| method(plugin, args)));
    reply(&result)
}

/// The turn position passed to `ooaw_set_turn`.
pub fn turn(game_turn: u32, step_index: u32, completed: u32) -> TurnPosition {
    TurnPosition {
        game_turn: game_turn as u16,
        step_index: step_index as usize,
        status: if completed == 0 {
            GameStatus::InProgress
        } else {
            GameStatus::Completed
        },
    }
}
