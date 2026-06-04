//! Property-based tests for the full-game keyframe codec (snapshot/restore).
//!
//! These drive a Game to a random state via arbitrary op sequences, then assert
//! the snapshot round-trips exactly and that post-restore continuation is
//! deterministic.

use bt_core::{Game, WeaponToken};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Op set — same as pbt.rs plus bazaar-relevant actions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Op {
    Left,
    Right,
    Rotate,
    Soft,
    Drop,
    Tick,
    ReceiveWeapon(usize), // index into WeaponToken::ALL
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => Just(Op::Tick),
        1 => Just(Op::Left),
        1 => Just(Op::Right),
        1 => Just(Op::Rotate),
        1 => Just(Op::Soft),
        1 => Just(Op::Drop),
        1 => (0usize..34).prop_map(Op::ReceiveWeapon),
    ]
}

fn apply(g: &mut Game, op: &Op) {
    match op {
        Op::Left => g.move_left(),
        Op::Right => g.move_right(),
        Op::Rotate => g.rotate(),
        Op::Soft => g.soft_drop(),
        Op::Drop => g.begin_drop(),
        Op::Tick => g.tick(16),
        Op::ReceiveWeapon(i) => g.receive_weapon(WeaponToken::ALL[*i]),
    }
}

// ---------------------------------------------------------------------------
// (a) ROUND-TRIP
//     Drive a fresh Game to a random state; let bytes = g.snapshot_bytes();
//     restore onto a Game created with a DIFFERENT seed; assert restored game's
//     snapshot_bytes() AND render_ids() equal the original's.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn keyframe_round_trip(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() { break; }
            apply(&mut g, o);
        }
        // Drain the bazaar so the game isn't frozen (makes post-restore ops
        // deterministic regardless of bazaar state).
        g.leave_bazaar();

        let bytes = g.snapshot_bytes();
        let ids = g.render_ids();

        // Restore into a game created with a DIFFERENT seed.
        let alt_seed = seed.wrapping_add(1);
        let mut h = Game::new(alt_seed);
        prop_assert!(h.restore_bytes(&bytes), "restore_bytes must accept a valid snapshot");
        prop_assert_eq!(h.snapshot_bytes(), bytes,
            "restored game must re-serialize identically");
        prop_assert_eq!(h.render_ids(), ids,
            "restored game render_ids must match original");
    }
}

// ---------------------------------------------------------------------------
// (b) POST-RESTORE DETERMINISM
//     The restored clone and the original, given the SAME ~20 inputs+ticks,
//     stay identical (render_ids + score) at each step.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn keyframe_post_restore_determinism(
        seed in any::<u64>(),
        setup_ops in prop::collection::vec(op(), 0..256),
        cont_ops in prop::collection::vec(op(), 0..20),
    ) {
        // Build the source game.
        let mut a = Game::new(seed);
        for o in &setup_ops {
            if a.is_game_over() { break; }
            apply(&mut a, o);
        }
        a.leave_bazaar();

        // Restore into b.
        let alt_seed = seed.wrapping_add(999);
        let mut b = Game::new(alt_seed);
        prop_assert!(b.restore_bytes(&a.snapshot_bytes()));

        // Drive both with the identical continuation.
        for o in &cont_ops {
            if a.is_game_over() && b.is_game_over() { break; }
            apply(&mut a, o);
            apply(&mut b, o);
            prop_assert_eq!(
                a.render_ids(), b.render_ids(),
                "render_ids diverged after {:?}", o,
            );
            prop_assert_eq!(
                a.score().score, b.score().score,
                "score diverged after {:?}", o,
            );
            prop_assert_eq!(
                a.score().funds, b.score().funds,
                "funds diverged after {:?}", o,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// (c) GARBAGE INPUT
//     restore_bytes on random junk byte vectors returns false and never
//     panics / never corrupts a usable game.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn garbage_restore_bytes_safe(
        seed in any::<u64>(),
        garbage in prop::collection::vec(any::<u8>(), 0..512),
    ) {
        let mut g = Game::new(seed);
        let before = g.snapshot_bytes();
        // This must not panic.
        let ok = g.restore_bytes(&garbage);
        if !ok {
            // State must be unchanged on rejection.
            prop_assert_eq!(g.snapshot_bytes(), before,
                "rejected restore must leave game untouched");
        }
        // Whether it succeeded or failed, the game must remain usable: tick
        // without panic.
        g.leave_bazaar();
        for _ in 0..10 {
            g.tick(16);
        }
    }
}

// ---------------------------------------------------------------------------
// (d) CLIENT KEYFRAME HIDES OP_FUNDS
//     client_keyframe_bytes() is identical to snapshot_bytes() except
//     op_funds is zeroed; restoring the client keyframe and checking the
//     score field confirms the redaction.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn client_keyframe_redacts_op_funds(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
        // A NON-ZERO op_funds so the redaction is actually observable — without
        // this the test was vacuous (op_funds defaults to 0, so a keyframe that
        // failed to redact would still pass).
        op_funds in 1i64..1_000_000,
        op_score in 0i64..1_000_000,
        op_lines in 0i64..1000,
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() { break; }
            apply(&mut g, o);
        }
        g.leave_bazaar();
        // Plant a real opponent-funds mirror to redact.
        g.receive_op_score(op_score, op_lines, op_funds);

        let full = g.snapshot_bytes();
        let client = g.client_keyframe_bytes();

        // Sanity: the FULL snapshot KEEPS op_funds (so the redaction check below
        // isn't vacuous — op_funds really was non-zero pre-keyframe).
        let mut h2 = Game::new(0);
        prop_assert!(h2.restore_bytes(&full));
        prop_assert_eq!(h2.score().op_funds, op_funds,
            "full snapshot must preserve op_funds");

        // The CLIENT keyframe must zero it.
        let mut h = Game::new(0);
        prop_assert!(h.restore_bytes(&client));
        prop_assert_eq!(h.score().op_funds, 0i64,
            "client keyframe must redact op_funds");

        // Every OTHER field is identical between client + full (only op_funds differs).
        prop_assert_eq!(h.score().score,    h2.score().score);
        prop_assert_eq!(h.score().funds,    h2.score().funds);
        prop_assert_eq!(h.score().lines,    h2.score().lines);
        prop_assert_eq!(h.score().op_score, h2.score().op_score);
        prop_assert_eq!(h.score().op_lines, h2.score().op_lines);
    }
}

// ---------------------------------------------------------------------------
// (e) ARSENAL survives the keyframe round-trip.
//     The round-trip tests above only ever drive `ReceiveWeapon`, which queues
//     incoming garbage rows / effects and NEVER touches the arsenal — so the
//     snapshot's arsenal section was always 20 empty ints, and a codec that
//     simply dropped it (serialised zeros) round-tripped vacuously. Here we
//     GRANT a random multiset of weapons into the arsenal first, then assert
//     every slot's (token, quantity) is preserved across snapshot→restore.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn keyframe_preserves_arsenal(
        seed in any::<u64>(),
        grants in prop::collection::vec(0usize..34, 1..40),
    ) {
        let mut g = Game::new(seed);
        for &i in &grants {
            g.grant_weapon(WeaponToken::ALL[i]);
        }
        // Guard against vacuity: the arsenal must actually hold something, else a
        // "drop the arsenal" codec mutant would pass on an already-empty arsenal.
        let nonempty = (0..10).any(|s| g.arsenal_token(s) >= 0);
        prop_assert!(nonempty, "precondition: granting >=1 weapon must fill an arsenal slot");

        let before: Vec<(i32, u16)> =
            (0..10).map(|s| (g.arsenal_token(s), g.arsenal_quantity(s))).collect();

        let bytes = g.snapshot_bytes();
        let mut h = Game::new(seed.wrapping_add(1));
        prop_assert!(h.restore_bytes(&bytes), "restore_bytes must accept a valid snapshot");

        let after: Vec<(i32, u16)> =
            (0..10).map(|s| (h.arsenal_token(s), h.arsenal_quantity(s))).collect();
        prop_assert_eq!(after, before,
            "arsenal (token + quantity per slot) must survive snapshot→restore");
    }
}
