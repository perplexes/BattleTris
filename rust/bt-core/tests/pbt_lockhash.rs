//! Property tests for the per-lock state hash (`Game::lock_seq` / `lock_hash`), the
//! divergence signal the model-(B) netcode resyncs on.
//!
//! Properties:
//!   (a) AGREEMENT: two games with the same seed and the same input stream hold
//!       identical `(lock_seq, lock_hash)` throughout. No false divergence, which is
//!       what makes a real divergence trustworthy.
//!   (b) FUNDS-INSENSITIVE: giving one of two otherwise-identical games extra funds
//!       does not change its `lock_hash`. Funds are the field a client is allowed to
//!       drift on between resyncs (the HUD reads them from the authoritative snapshot),
//!       so a funds difference must not read as divergence.
//!   (c) ROUND-TRIP: `lock_seq` / `lock_hash` survive a snapshot / restore, so a
//!       resynced client resumes the check at the right lock.
//!   (d) DETECTION: a real board divergence changes the hash (a concrete construction;
//!       the 32-bit hash makes this near-certain rather than guaranteed, so it is a
//!       unit test on a chosen divergence, not a property over random inputs).

use bt_core::Game;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Left,
    Right,
    Rotate,
    Drop,
    Tick,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        5 => Just(Op::Tick),
        1 => Just(Op::Left),
        1 => Just(Op::Right),
        1 => Just(Op::Rotate),
        2 => Just(Op::Drop),
    ]
}

fn apply(g: &mut Game, o: &Op) {
    match o {
        Op::Left => g.move_left(),
        Op::Right => g.move_right(),
        Op::Rotate => g.rotate(),
        Op::Drop => g.begin_drop(),
        Op::Tick => g.tick(16),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// (a) AGREEMENT.
    #[test]
    fn same_seed_same_inputs_agree_on_lock_hash(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..400),
    ) {
        let mut a = Game::new(seed);
        let mut b = Game::new(seed);
        for o in &ops {
            apply(&mut a, o);
            apply(&mut b, o);
            prop_assert_eq!(a.lock_seq(), b.lock_seq());
            prop_assert_eq!(a.lock_hash(), b.lock_hash());
        }
    }

    /// (b) FUNDS-INSENSITIVE.
    #[test]
    fn funds_do_not_affect_the_lock_hash(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..400),
        inject in 1i64..1_000_000,
    ) {
        let mut a = Game::new(seed);
        let mut b = Game::new(seed);
        let mut injected = false;
        for o in &ops {
            apply(&mut a, o);
            apply(&mut b, o);
            // On the first drop, give A extra funds. Nothing else about A differs.
            if !injected && matches!(o, Op::Drop) {
                a.add_funds(inject);
                injected = true;
            }
            prop_assert_eq!(a.lock_seq(), b.lock_seq());
            prop_assert_eq!(a.lock_hash(), b.lock_hash(), "funds must not change the lock hash");
        }
        if injected {
            prop_assert_ne!(a.score().funds, b.score().funds, "funds genuinely diverged");
        }
    }

    /// (c) ROUND-TRIP.
    #[test]
    fn lock_fields_survive_snapshot_restore(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..400),
        seed2 in any::<u64>(),
    ) {
        let mut a = Game::new(seed);
        for o in &ops {
            apply(&mut a, o);
        }
        let snap = a.snapshot();
        let mut b = Game::new(seed2); // a different starting state
        prop_assert!(b.restore(&snap));
        prop_assert_eq!(a.lock_seq(), b.lock_seq());
        prop_assert_eq!(a.lock_hash(), b.lock_hash());
        // The restored game re-serializes identically, so the stored hash is consistent
        // with the rest of the restored state.
        prop_assert_eq!(b.snapshot(), a.snapshot());
    }
}

/// (d) DETECTION.
#[test]
fn a_board_divergence_changes_the_hash() {
    // Same seed gives both games the identical piece stream. Drop the first piece
    // against the left wall in A and the right wall in B, so the locked boards differ.
    let mut a = Game::new(12345);
    let mut b = Game::new(12345);
    for _ in 0..6 {
        a.move_left();
        b.move_right();
    }
    a.begin_drop();
    b.begin_drop();
    for _ in 0..400 {
        a.tick(16);
        b.tick(16);
        if a.lock_seq() >= 1 && b.lock_seq() >= 1 {
            break;
        }
    }
    assert_eq!(a.lock_seq(), b.lock_seq(), "same drop cadence -> same lock count");
    assert_ne!(a.export_board(), b.export_board(), "the locked boards must actually differ");
    assert_ne!(a.lock_hash(), b.lock_hash(), "a board divergence must change the hash");
}
