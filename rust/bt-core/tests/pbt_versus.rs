//! Property-based tests for the cross-player Versus relay.
//!
//! Properties:
//!   (a) DETERMINISM: same seeds + same op stream → identical board renders and
//!       identical result() on two independent Versus instances.
//!   (b) RESULT LATCH MONOTONIC: once result()!=0 it never returns to 0 or
//!       changes to the other winner on subsequent ticks.
//!   (c) RELAY CONSERVATION (Swap): when a Swap is delivered, total filled
//!       cells across both boards is conserved.

use bt_core::versus::Side;
use bt_core::{Versus, WeaponToken};
use proptest::prelude::*;

// ---- shared op type ---------------------------------------------------------

#[derive(Debug, Clone)]
enum Op {
    // Per-side inputs — (side A, side B) encoded as one enum variant each.
    LeftA,
    RightA,
    RotateA,
    DropA,
    LeftB,
    RightB,
    RotateB,
    DropB,
    Tick,
    /// A fires RiseUp at B (grant + launch in one step via grant_weapon +
    /// launch_weapon so the relay handles the delivery).
    AFiresRiseUp,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        8 => Just(Op::Tick),
        1 => Just(Op::LeftA),
        1 => Just(Op::RightA),
        1 => Just(Op::RotateA),
        1 => Just(Op::DropA),
        1 => Just(Op::LeftB),
        1 => Just(Op::RightB),
        1 => Just(Op::RotateB),
        1 => Just(Op::DropB),
        1 => Just(Op::AFiresRiseUp),
    ]
}

fn apply(v: &mut Versus, op: &Op) {
    match op {
        Op::Tick => v.tick(16),
        Op::LeftA => v.game_mut(Side::A).move_left(),
        Op::RightA => v.game_mut(Side::A).move_right(),
        Op::RotateA => v.game_mut(Side::A).rotate(),
        Op::DropA => v.game_mut(Side::A).begin_drop(),
        Op::LeftB => v.game_mut(Side::B).move_left(),
        Op::RightB => v.game_mut(Side::B).move_right(),
        Op::RotateB => v.game_mut(Side::B).rotate(),
        Op::DropB => v.game_mut(Side::B).begin_drop(),
        Op::AFiresRiseUp => {
            // Only fire if A is still alive and has room in its arsenal.
            if !v.game(Side::A).is_game_over() {
                v.game_mut(Side::A).grant_weapon(WeaponToken::RiseUp);
                v.game_mut(Side::A).launch_weapon(0);
                // tick so relay delivers it
                v.tick(16);
            }
        }
    }
}

// ---- (a) DETERMINISM --------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Two Versus instances with identical seeds produce bit-for-bit identical
    /// board exports and identical result() after the same op stream.
    #[test]
    fn versus_is_deterministic(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
    ) {
        let mut v1 = Versus::new(seed_a, seed_b);
        let mut v2 = Versus::new(seed_a, seed_b);

        for o in &ops {
            if v1.is_over() {
                break;
            }
            apply(&mut v1, o);
            apply(&mut v2, o);
        }

        prop_assert_eq!(
            v1.game(Side::A).export_board(),
            v2.game(Side::A).export_board(),
            "Side A boards must be identical"
        );
        prop_assert_eq!(
            v1.game(Side::B).export_board(),
            v2.game(Side::B).export_board(),
            "Side B boards must be identical"
        );
        prop_assert_eq!(
            v1.result(),
            v2.result(),
            "result() must be identical"
        );
    }
}

// ---- (b) RESULT LATCH MONOTONIC ---------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Once result() becomes non-zero it must never go back to 0 or change to
    /// the opposite winner, regardless of how many more ticks are applied.
    #[test]
    fn result_latch_is_monotonic(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
        extra_ticks in 0usize..64,
    ) {
        let mut v = Versus::new(seed_a, seed_b);

        for o in &ops {
            apply(&mut v, o);
        }

        let result_at_end_of_ops = v.result();
        if result_at_end_of_ops == 0 {
            // Game still ongoing — apply more ticks until it ends or we exhaust
            // extra_ticks, just to exercise the latch path.
            for _ in 0..extra_ticks {
                v.tick(16);
            }
            // Nothing to assert for the latch since it may not have fired.
            return Ok(());
        }

        let winner = result_at_end_of_ops;
        // Now apply extra ticks; result must stay the same.
        for _ in 0..extra_ticks {
            v.tick(16);
            prop_assert_eq!(
                v.result(),
                winner,
                "result() changed after latch: was {} now {}",
                winner,
                v.result()
            );
        }
    }
}

// ---- (c) RELAY CONSERVATION: Swap exchanges boards exactly ------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// When Swap is delivered via the relay, each side ends up with exactly
    /// the other side's board grid. Swap is a pure exchange — A's export
    /// becomes B's board and vice versa — with no cells created or destroyed.
    ///
    /// We verify this at the board-export level (board grid only, not the
    /// falling piece) immediately before and after a forced Swap delivery.
    /// We use `swap_board_with` directly (the same underlying function the
    /// relay calls) so we isolate the board exchange invariant from the
    /// timing of `tick` + piece-lock side-effects.
    #[test]
    fn swap_exchanges_boards_exactly(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        ops in prop::collection::vec(op(), 0..128),
    ) {
        let mut v = Versus::new(seed_a, seed_b);

        // Drive both boards to random state.
        for o in &ops {
            if v.is_over() { break; }
            apply(&mut v, o);
        }

        if v.is_over() {
            return Ok(());
        }

        // Snapshot both board grids before Swap.
        let board_a_before = v.game(Side::A).export_board();
        let board_b_before = v.game(Side::B).export_board();

        // Perform the Swap via the public relay path: grant Swap to A and
        // launch it, then tick so the relay's deliver_weapon fires.
        // We need to launch it from slot 0 specifically; grant gives us one.
        v.game_mut(Side::A).grant_weapon(WeaponToken::Swap);
        // Find the slot that now holds Swap.
        let swap_slot = (0..10usize).find(|&i| {
            v.game(Side::A).arsenal_token(i) == WeaponToken::Swap.index() as i32
        });
        let Some(slot) = swap_slot else { return Ok(()); };
        v.game_mut(Side::A).launch_weapon(slot);
        // relay() runs inside tick() — the Swap fires synchronously in relay,
        // but the tick also advances game time (may lock pieces). Capture
        // boards immediately after relay fires by using tick(0) if available,
        // or just accept tick(1) as close enough. We check the board grids
        // BEFORE the tick (snapshot above) vs AFTER, knowing the relay has
        // already swapped by end of tick.
        //
        // Actually: Swap is an instant weapon applied in relay synchronously
        // within tick. The piece-lock timing within the same tick could add cells
        // to the ALREADY-SWAPPED board, so the comparison we really want is:
        //   A_after == B_before  AND B_after == A_before
        // only if no piece locking happened in that tick. Instead we verify
        // the weaker but correct property: A_after.len() == B_before.len() AND
        // B_after.len() == A_before.len() in terms of filled cell counts.
        // But that is also broken by locking. The cleanest correct property
        // that doesn't depend on lock timing is: the sets of export bytes are
        // swapped — i.e. the relay's exchange is a pure bijection at the cell level.
        //
        // We achieve this by using tick(0) — a zero-dt tick that runs relay but
        // does not advance game physics (no piece movement / locking).
        v.tick(0);

        let board_a_after = v.game(Side::A).export_board();
        let board_b_after = v.game(Side::B).export_board();

        prop_assert_eq!(
            &board_a_after, &board_b_before,
            "after Swap, A's board must equal B's board before Swap"
        );
        prop_assert_eq!(
            &board_b_after, &board_a_before,
            "after Swap, B's board must equal A's board before Swap"
        );
    }
}
