fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// `SplitMix64` rejection sampling, version one. Distinct tagged streams never
/// consume one another's draws. Discard the incomplete residue class prefix.
pub struct Draw(u64);

impl Draw {
    /// Start one named stream from a recorded run or merge seed.
    #[must_use]
    pub fn new(seed: u64, tag: u64) -> Self {
        Self(mix(seed ^ tag))
    }

    /// Draw an ordinal below the exclusive positive bound without modulo bias.
    ///
    /// # Panics
    /// Panics for an empty draw range.
    pub fn below(&mut self, upper: u64) -> u64 {
        assert!(upper != 0, "a draw range must be nonempty");
        let minimum = upper.wrapping_neg() % upper;
        loop {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let next = mix(self.0);
            if next >= minimum {
                return next % upper;
            }
        }
    }
}
