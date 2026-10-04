//! Deterministic dice for reproducible games, saves, and replays.

/// SplitMix64 generator seeded from the game identifier.
///
/// The kernel owns the only dice of a game; plugins roll them through the
/// `roll` import, so every random outcome is reproducible from the seed and
/// the command history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dice {
    /// Current SplitMix64 state, advanced for every generated random value.
    state: u64,
}

impl Dice {
    /// Seeds the generator from a stable string such as the game ID (FNV-1a).
    pub fn from_seed_text(text: &str) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self { state: hash }
    }

    /// Advances the SplitMix64 state and returns the next deterministic 64-bit random value.
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Rolls one die with `sides` faces, returning a value from 1 to `sides`.
    ///
    /// # Panics
    ///
    /// Panics if `sides` is zero.
    pub fn roll(&mut self, sides: u32) -> u32 {
        assert!(sides > 0, "a die needs at least one side");
        let sides = u64::from(sides);
        // Rejection sampling keeps the roll exactly uniform.
        let limit = u64::MAX - u64::MAX % sides;
        loop {
            let value = self.next_u64();
            if value < limit {
                return (value % sides) as u32 + 1;
            }
        }
    }

    /// Rolls one six-sided die.
    pub fn d6(&mut self) -> u8 {
        self.roll(6) as u8
    }
}
