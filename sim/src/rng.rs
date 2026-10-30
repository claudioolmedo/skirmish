//! Hand-rolled seeded RNG (PCG32, XSH-RR output).
//!
//! Written from scratch for Skirmish — no external RNG crate is used on
//! purpose: third-party RNG implementations may change their output between
//! releases, which would silently break cross-version replay determinism.
//! The generator state lives *inside* the sim state and is hashed every tick,
//! so any divergence is caught immediately by the determinism harness.
//!
//! Algorithm: PCG-XSH-RR 64/32 (LCG 2^64). Constants are the published PCG
//! defaults: multiplier 6364136223846793005, increment forced odd.

/// PCG32 generator: 64 bits of state, 32 bits of output per draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    const MULT: u64 = 6_364_136_223_846_793_005;

    /// Canonical PCG seeding: the increment (stream selector) is derived from
    /// the seed and forced odd; the state is then mixed twice.
    pub fn new(seed: u64) -> Self {
        let mut rng = Pcg32 {
            state: 0,
            inc: (seed << 1) | 1,
        };
        rng.step();
        rng.state = rng.state.wrapping_add(seed);
        rng.step();
        rng
    }

    #[inline]
    fn step(&mut self) {
        self.state = self.state.wrapping_mul(Self::MULT).wrapping_add(self.inc);
    }

    /// Next uniformly distributed u32 (XSH-RR output function).
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.step();
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Next u64, composed of two u32 draws (high word first).
    pub fn next_u64(&mut self) -> u64 {
        ((self.next_u32() as u64) << 32) | (self.next_u32() as u64)
    }

    /// Uniform value in `[0, bound)` via rejection sampling.
    ///
    /// Deterministic for a given generator state; the loop always terminates.
    pub fn below(&mut self, bound: u32) -> u32 {
        assert!(bound > 0, "Pcg32::below requires bound > 0");
        // Values below this threshold would make `r % bound` biased;
        // redraw instead. (2^32 - bound) % bound, computed without overflow.
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return r % bound;
            }
        }
    }

    /// Raw generator words, for inclusion in the per-tick state hash.
    pub fn state_words(&self) -> (u64, u64) {
        (self.state, self.inc)
    }

    /// Rebuild a generator from raw words (NET-09 save/restore):
    /// the exact inverse of `state_words` — a restored generator
    /// continues the stream bit-for-bit.
    pub fn from_words(state: u64, inc: u64) -> Self {
        Pcg32 { state, inc }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Pcg32::new(99);
        let mut b = Pcg32::new(99);
        for _ in 0..1000 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Pcg32::new(1);
        let mut b = Pcg32::new(2);
        assert_ne!(a.next_u32(), b.next_u32());
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = Pcg32::new(7);
        for _ in 0..10_000 {
            let v = rng.below(137);
            assert!(v < 137);
        }
    }
}
