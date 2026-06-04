//! Property-based tests for the full-game keyframe codec (snapshot/restore).
//!
//! These drive a Game to a random state via arbitrary op sequences, then assert
//! the snapshot round-trips exactly and that post-restore continuation is
//! deterministic.

use bt_core::{Cell, Game, WeaponToken};
use proptest::prelude::*;

/// Force at least one REAL line clear through the engine: pre-fill the bottom two
/// rows with die boxes (so the clear awards genuine lines+funds), then drop and
/// tick until the falling piece locks — `place` runs `check_lines`, which clears
/// those full rows and bumps `score.lines` / `score.funds` / `lines_til_bazaar`.
/// Returns true if a clear actually happened.
fn force_a_line_clear(g: &mut Game) -> bool {
    {
        let b = g.board_mut();
        let (w, h) = (b.width, b.height);
        for y in [h - 1, h - 2] {
            for x in 0..w {
                b.set(x, y, Some(Cell::die(6)));
            }
        }
    }
    let lines_before = g.score().lines;
    g.begin_drop();
    for _ in 0..1200 {
        g.tick(16);
        g.take_events();
        if g.score().lines > lines_before {
            return true;
        }
        if g.is_game_over() {
            return false;
        }
    }
    g.score().lines > lines_before
}

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
// (a') FULL-FIELD ROUND-TRIP. The byte-equality check above proves the codec is
//      self-consistent, but it can't catch a field that's WRONG on both the
//      serialize and deserialize side in the same way (e.g. always writing
//      `score.lines` as 0 — it round-trips vacuously). Here we read each semantic
//      field back through its OWN accessor and compare to the source, AND we
//      first force a real line clear + plant op_* + a duration weapon so none of
//      the fields is its default. A codec that serialises `score.lines` (or
//      op_*/funds/lines_til_bazaar/weapon_remaining) as 0 now fails.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn keyframe_preserves_full_score_and_durations(
        seed in any::<u64>(),
        op_score in 1i64..1_000_000,
        op_lines in 1i64..10_000,
        op_funds in 1i64..1_000_000,
        // a duration weapon to receive (any token; we just need a non-zero
        // `remaining` for SOME weapon).
        dur_weapon in 0usize..34,
    ) {
        let mut g = Game::new(seed);
        // Real line clear -> non-zero score.lines / funds / lines_til_bazaar shift.
        let cleared = force_a_line_clear(&mut g);
        // Plant the opponent mirror fields (op_score/op_lines/op_funds).
        g.receive_op_score(op_score, op_lines, op_funds);
        // Receive a weapon and lock once to FLUSH it so `remaining[token]` is set.
        let tok = WeaponToken::ALL[dur_weapon];
        g.receive_weapon(tok);
        // Drive a lock to apply the pending weapon (its duration accumulates into
        // `remaining`). Re-fill rows so the lock also keeps score moving.
        let _ = force_a_line_clear(&mut g);
        g.leave_bazaar();

        // Snapshot the FULL semantic state via the public accessors.
        let s = g.score();
        let ltb = g.lines_til_bazaar();
        let rems: Vec<i32> = WeaponToken::ALL.iter().map(|&t| g.weapon_remaining(t)).collect();

        // Sanity (non-vacuity): the planted op_* really took, and we really
        // cleared at least one line.
        prop_assert_eq!((s.op_score, s.op_lines, s.op_funds), (op_score, op_lines, op_funds),
            "op_* must reflect the planted receive_op_score");
        prop_assert!(cleared, "the test must force a real line clear (score.lines > 0)");
        prop_assert!(s.lines > 0, "score.lines must be non-zero after a real clear");

        // Round-trip into a DIFFERENT-seed game.
        let bytes = g.snapshot_bytes();
        let mut h = Game::new(seed.wrapping_add(7));
        prop_assert!(h.restore_bytes(&bytes), "restore_bytes must accept a valid snapshot");

        // Every semantic field must survive — read through the SAME accessors.
        prop_assert_eq!(h.score(), s, "the full Score (incl. lines, op_*, funds) must round-trip");
        prop_assert_eq!(h.lines_til_bazaar(), ltb, "lines_til_bazaar must round-trip");
        let rems2: Vec<i32> = WeaponToken::ALL.iter().map(|&t| h.weapon_remaining(t)).collect();
        prop_assert_eq!(rems2, rems, "per-weapon remaining durations must round-trip");
    }
}

// ---------------------------------------------------------------------------
// (a'') RESTORE RETURN VALUE on EXPLICIT malformed inputs. The garbage tests
//       ignore restore_bytes' bool, so a mutant making a bad-version / wrong-
//       length keyframe `return true` (then committing junk) survives. Pin the
//       contract directly: a valid snapshot returns true; a bumped version word,
//       a truncated buffer, an extra trailing word, and a non-multiple-of-8
//       length each return false AND leave the game untouched.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn restore_rejects_malformed_keyframes(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..128),
    ) {
        let mut src = Game::new(seed);
        for o in &ops {
            if src.is_game_over() { break; }
            apply(&mut src, o);
        }
        src.leave_bazaar();
        let good = src.snapshot_bytes();

        // A valid snapshot MUST be accepted.
        {
            let mut h = Game::new(seed.wrapping_add(1));
            prop_assert!(h.restore_bytes(&good), "a valid snapshot must restore (return true)");
        }

        // Helper: a malformed buffer must be rejected AND leave the target intact.
        let check_reject = |bytes: &[u8]| -> Result<(), TestCaseError> {
            let mut h = Game::new(seed.wrapping_add(2));
            let before = h.snapshot_bytes();
            let ok = h.restore_bytes(bytes);
            prop_assert!(!ok, "restore_bytes must REJECT a malformed keyframe (returned true)");
            prop_assert_eq!(h.snapshot_bytes(), before,
                "a rejected restore must leave the game byte-identical");
            Ok(())
        };

        // 1) Bad VERSION word: the first i64 (LE) is the keyframe version; bump it.
        {
            let mut bad = good.clone();
            // First 8 bytes hold the version little-endian; perturb the low byte.
            bad[0] = bad[0].wrapping_add(1);
            check_reject(&bad)?;
        }
        // 2) TRUNCATED: drop the last word (still a multiple of 8 -> passes the
        //    length gate, fails the "fully consumed" check).
        if good.len() >= 8 {
            check_reject(&good[..good.len() - 8])?;
        }
        // 3) TRAILING garbage: append an extra word (consumes short -> trailing).
        {
            let mut bad = good.clone();
            bad.extend_from_slice(&0i64.to_le_bytes());
            check_reject(&bad)?;
        }
        // 4) NON-multiple-of-8 length: the byte-length gate must reject it.
        {
            let mut bad = good.clone();
            bad.push(0u8);
            check_reject(&bad)?;
        }
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

        // STRONGER: the client keyframe must equal the FULL snapshot with ONLY
        // op_funds zeroed — byte for byte, across the WHOLE state (board, arsenal,
        // pieces, weapon durations, …). The score-only checks above can't catch a
        // client keyframe that ALSO mangles non-score state (e.g. a stray
        // `g.board.clear()` inside `client_keyframe_bytes`): build the expected
        // redacted bytes by taking the full snapshot, zeroing JUST op_funds, and
        // re-serialising, then demand the client keyframe matches exactly.
        let mut expected = Game::new(seed.wrapping_add(31));
        prop_assert!(expected.restore_bytes(&full));
        // Re-plant op_* with op_funds = 0 (score-mirror set has no other side effects).
        expected.receive_op_score(op_score, op_lines, 0);
        prop_assert_eq!(expected.snapshot_bytes(), client.clone(),
            "client keyframe must equal the full snapshot with ONLY op_funds redacted \
             (anything else mangled — e.g. the board cleared — is a leak/bug)");
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

// ---------------------------------------------------------------------------
// (e') ARSENAL HOLES + quantities survive the keyframe round-trip.
//     `grant_weapon` only ever fills CONTIGUOUS slots, so (e) couldn't catch a
//     restore that compacts holes (e.g. rebuild via Arsenal::buy instead of
//     import_arsenal). Here we plant a deliberately HOLEY layout via
//     import_arsenal — a filled slot, an empty slot, a qty>1 slot, another hole —
//     and assert the EXACT per-slot layout (incl. the holes between fills) round
//     trips. A compaction would shift slot 2's weapon into slot 1 and fail.
// ---------------------------------------------------------------------------

#[test]
fn keyframe_preserves_holey_arsenal() {
    let mut g = Game::new(0xA15E);
    // [token, qty] per slot; token -1 = empty. Hole at slot 1 and 3; qty>1 at 2 & 4.
    #[rustfmt::skip]
    let layout: Vec<i32> = vec![
        5, 1,    // slot 0
        -1, 0,   // slot 1  <- HOLE before a fill
        7, 4,    // slot 2  <- qty > 1
        -1, 0,   // slot 3  <- HOLE between fills
        2, 3,    // slot 4  <- qty > 1
        -1, 0, -1, 0, -1, 0, -1, 0, -1, 0,  // slots 5..9 empty
    ];
    g.import_arsenal(&layout);

    // Sanity: the holey layout actually took (else the test is vacuous).
    assert_eq!(g.arsenal_token(0), 5);
    assert_eq!(g.arsenal_token(1), -1, "slot 1 must be a genuine hole, not compacted away");
    assert_eq!((g.arsenal_token(2), g.arsenal_quantity(2)), (7, 4));
    assert_eq!(g.arsenal_token(3), -1, "slot 3 must be a genuine hole");

    let before: Vec<(i32, u16)> =
        (0..10).map(|s| (g.arsenal_token(s), g.arsenal_quantity(s))).collect();

    let bytes = g.snapshot_bytes();
    let mut h = Game::new(1);
    assert!(h.restore_bytes(&bytes), "restore_bytes must accept a valid snapshot");

    let after: Vec<(i32, u16)> =
        (0..10).map(|s| (h.arsenal_token(s), h.arsenal_quantity(s))).collect();
    assert_eq!(after, before,
        "holey arsenal (incl. the gaps between filled slots) must survive snapshot→restore");
}
