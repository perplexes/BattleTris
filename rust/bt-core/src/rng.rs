//! Deterministic, seedable RNG reproducing the POSIX `drand48` family and the
//! `rand()` the game draws from.
//!
//! Determinism is the whole point: the engine is replayed and re-simulated
//! (replays, property tests, and server-authoritative reconciliation all rerun
//! the same seed and must land on the same state), so randomness has to be a
//! pure function of the seed and the exact sequence of draws — never the host
//! platform's libc. This is a single 48-bit LCG implemented per POSIX so it
//! gives identical results everywhere.
//!
//! The order and KIND of draw matters as much as the values, because the engine
//! consumes them in a fixed order that any faithful re-run must match:
//!   * piece selection rolls `rand()` for the id, then `drand48()` against the
//!     keep probability, plus `lrand48()` for the Broken Record reroll.
//!   * a die's pips come from `rand()`.
//!   * board weapon effects draw `rand()` for positions and coin flips.
//!
//! Each generator pulls from the same shared 48-bit state, so the methods are
//! not interchangeable — calling the wrong one would desync every later draw.

/// Upper bound of [`Rng::rand`], inclusive — the 31-bit range the game's modulo
/// idioms (`rand() % n`) assume.
pub const RAND_MAX: i32 = 0x7fff_ffff;

/// A deterministic POSIX-style RNG. All three generators share one 48-bit LCG
/// state, so the struct is cheap to clone and snapshot (a single integer is the
/// entire reproducible random stream).
#[derive(Clone, Debug)]
pub struct Rng {
    /// The 48-bit LCG state. Only the low 48 bits are significant; the top 16
    /// are always zero after each step.
    state: u64,
}

impl Rng {
    /// Seed the generator (`srand48(seed)` semantics).
    pub fn new(seed: u64) -> Rng {
        // POSIX srand48(seed): set the high 32 bits of the 48-bit state to the
        // low 32 bits of `seed`, and the low 16 bits to 0x330E.
        // X = ((seed & 0xFFFF_FFFF) << 16) | 0x330E
        let state = ((seed & 0xFFFF_FFFF) << 16) | 0x330E;
        Rng { state }
    }

    /// The raw 48-bit LCG state — for full-game keyframe serialization (the
    /// client-server reconciliation snapshot). Pair with [`Rng::from_raw`].
    pub fn raw(&self) -> u64 {
        self.state
    }

    /// Rebuild from a raw state captured by [`Rng::raw`].
    pub fn from_raw(state: u64) -> Rng {
        Rng { state }
    }

    /// The single LCG step shared by every generator: `X = (A·X + C) mod 2^48`,
    /// the POSIX `drand48` recurrence. Every public draw advances state exactly
    /// once through here, which is what keeps the stream order well-defined.
    fn next_state(&mut self) -> u64 {
        const A: u64 = 0x5DEECE66D;
        const C: u64 = 0xB;
        const MOD: u64 = 1u64 << 48;
        self.state = (A.wrapping_mul(self.state).wrapping_add(C)) & (MOD - 1);
        self.state
    }

    /// `rand()` — uniform in `0..=RAND_MAX`. Takes the top 31 bits of the 48-bit
    /// state, the high bits being the well-distributed ones in an LCG.
    pub fn rand(&mut self) -> i32 {
        let x = self.next_state();
        (x >> 17) as i32
    }

    /// `rand() % n`, the modulo idiom the game uses for "pick one of `n`".
    /// Requires `n > 0`.
    pub fn rand_below(&mut self, n: i32) -> i32 {
        self.rand() % n
    }

    /// `drand48()` — uniform double in `[0.0, 1.0)`, the full 48-bit state scaled
    /// to a fraction. Used for the keep-probability comparison in piece
    /// selection.
    pub fn drand48(&mut self) -> f64 {
        let x = self.next_state();
        x as f64 / ((1u64 << 48) as f64)
    }

    /// `lrand48()` — uniform non-negative long in `0..2^31` (same top-31-bit
    /// extraction as [`Rng::rand`], returned wide). Used for the Broken Record
    /// reroll.
    pub fn lrand48(&mut self) -> i64 {
        let x = self.next_state();
        (x >> 17) as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determinism() {
        // Two Rng instances with the same seed produce identical sequences.
        let mut rng1 = Rng::new(12345);
        let mut rng2 = Rng::new(12345);

        for _ in 0..100 {
            assert_eq!(rng1.rand(), rng2.rand());
        }

        let mut rng1 = Rng::new(12345);
        let mut rng2 = Rng::new(12345);

        for _ in 0..100 {
            assert_eq!(rng1.drand48(), rng2.drand48());
        }

        let mut rng1 = Rng::new(12345);
        let mut rng2 = Rng::new(12345);

        for _ in 0..100 {
            assert_eq!(rng1.lrand48(), rng2.lrand48());
        }
    }

    #[test]
    fn test_different_seeds() {
        let mut rng1 = Rng::new(12345);
        let mut rng2 = Rng::new(54321);

        // Different seeds should produce different first values.
        assert_ne!(rng1.rand(), rng2.rand());

        let mut rng1 = Rng::new(12345);
        let mut rng2 = Rng::new(54321);
        assert_ne!(rng1.drand48(), rng2.drand48());

        let mut rng1 = Rng::new(12345);
        let mut rng2 = Rng::new(54321);
        assert_ne!(rng1.lrand48(), rng2.lrand48());
    }

    #[test]
    fn test_drand48_range() {
        let mut rng = Rng::new(12345);
        for _ in 0..10_000 {
            let val = rng.drand48();
            assert!((0.0..1.0).contains(&val), "drand48() value {} out of range", val);
        }
    }

    #[test]
    fn test_lrand48_range() {
        let mut rng = Rng::new(12345);
        for _ in 0..10_000 {
            let val = rng.lrand48();
            assert!(val >= 0 && val <= RAND_MAX as i64, "lrand48() value {} out of range", val);
        }
    }

    #[test]
    fn test_rand_range() {
        let mut rng = Rng::new(12345);
        for _ in 0..10_000 {
            let val = rng.rand();
            // Widen to i64 for the upper bound: RAND_MAX == i32::MAX, so `val <= RAND_MAX`
            // as i32 is vacuously true (clippy::absurd_extreme_comparisons). The i64 form
            // still documents rand()'s `0..=RAND_MAX` contract — matching test_lrand48_range.
            assert!(val >= 0 && val as i64 <= RAND_MAX as i64, "rand() value {} out of range", val);
        }
    }

    #[test]
    fn test_rand_below_die_roll() {
        // rand_below(6) + 1 should be in 1..=6
        let mut rng = Rng::new(12345);
        for _ in 0..10_000 {
            let val = rng.rand_below(6) + 1;
            assert!((1..=6).contains(&val), "die roll {} out of range", val);
        }
    }

    #[test]
    fn test_rand_below_range() {
        // rand_below(10) should be in 0..10
        let mut rng = Rng::new(12345);
        for _ in 0..10_000 {
            let val = rng.rand_below(10);
            assert!((0..10).contains(&val), "rand_below(10) value {} out of range", val);
        }
    }

    #[test]
    fn test_lcg_step_verification() {
        // Verify the LCG step after seeding with 0.
        let mut rng = Rng::new(0);

        // After seeding with 0, state should be 0x330E.
        assert_eq!(rng.state, 0x330E);

        // The first next_state() should compute:
        // (A * 0x330E + C) & (2^48 - 1)
        // where A = 0x5DEECE66D, C = 0xB
        const A: u64 = 0x5DEECE66D;
        const C: u64 = 0xB;
        const MOD: u64 = 1u64 << 48;
        let expected = (A.wrapping_mul(0x330E).wrapping_add(C)) & (MOD - 1);

        // Call rand() to trigger next_state() and verify.
        let _ = rng.rand();
        assert_eq!(rng.state, expected, "LCG step mismatch");
    }
}
