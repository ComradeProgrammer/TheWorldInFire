//! The game's dice, which belong to the kernel.

use ooaw_plugin_sdk::host;

/// Handle to the kernel's seeded dice.
///
/// Rolling asks the kernel for the next value, so every roll is reproducible
/// from the game's seed and command history.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Dice;

impl Dice {
    /// Rolls one six-sided die.
    pub(crate) fn d6(&mut self) -> u8 {
        host::roll(6)
    }

    /// Rolls one twenty-sided die.
    pub(crate) fn d20(&mut self) -> u8 {
        host::roll(20)
    }
}
