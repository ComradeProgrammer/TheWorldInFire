//! Deterministic dice for reproducible games, saves, and replays.

/// SplitMix64 generator seeded from the game identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dice {
    state: u64,
}

impl Dice {
    /// Seeds the generator from a stable string such as the game ID (FNV-1a).
    pub(crate) fn from_seed_text(text: &str) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self { state: hash }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Rolls one six-sided die.
    pub(crate) fn d6(&mut self) -> u8 {
        // Rejection sampling keeps the roll exactly uniform.
        const LIMIT: u64 = u64::MAX - u64::MAX % 6;
        loop {
            let value = self.next_u64();
            if value < LIMIT {
                return (value % 6) as u8 + 1;
            }
        }
    }
}
