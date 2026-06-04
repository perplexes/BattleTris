//! Property-based tests for the cross-player Versus relay.
//!
//! Properties:
//!   (a) DETERMINISM: same seeds + same op stream → identical board renders and
//!       identical result() on two independent Versus instances.
//!   (b) RESULT LATCH MONOTONIC: once result()!=0 it never returns to 0 or
//!       changes to the other winner on subsequent ticks.
//!   (c) RELAY CONSERVATION (Swap): when a Swap is delivered, total filled
//!       cells across both boards is conserved.

use bt_core::versus::{deliver_weapon, Side};
use bt_core::{Cell, Game, GameEvent, Versus, WeaponToken};
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

// ---- (b') RESULT LATCH after a FORCED top-out -------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Force a real top-out (random play almost never reaches one), then verify
    /// the latch holds. Side B's board is filled near-full with a DIAGONAL hole
    /// so no row is ever complete — nothing clears it away — so B cannot place
    /// pieces and tops out; result() then must stay fixed for all further play.
    #[test]
    fn result_latches_after_forced_topout(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        extra in 1usize..120usize,
    ) {
        let mut v = Versus::new(seed_a, seed_b);
        {
            let b = v.game_mut(Side::B).board_mut();
            let (w, h) = (b.width, b.height);
            for y in 4..h {
                for x in 0..w {
                    if x != y % w {            // one empty cell per row -> never a full line
                        b.set(x, y, Some(Cell::color(1)));
                    }
                }
            }
        }

        // Drive B down until it tops out.
        let mut r = 0;
        for _ in 0..800 {
            v.game_mut(Side::B).begin_drop();
            v.tick(16);
            r = v.result();
            if r != 0 { break; }
        }
        prop_assert!(r != 0, "B did not top out from a near-full board");

        // Latched: result is unchanged by any further ticks.
        for _ in 0..extra {
            v.tick(16);
            prop_assert_eq!(v.result(), r, "result changed after latching to {}", r);
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

// ---------------------------------------------------------------------------
// (d) PER-WEAPON RELAY EFFECTS.
//
// The relay tests above only exercise RiseUp + a forced Swap, so the routing of
// every OTHER cross-player weapon was unpinned: a mutant `Susan => swap_board_with`
// (instead of swap_arsenal_with), or one sending Keating/Mondale to the wrong
// side, sailed through. These pin each weapon's distinct relay effect.
// ---------------------------------------------------------------------------

/// Drive a single piece to lock (so queued weapons / funds effects flush).
fn lock(g: &mut Game) {
    g.begin_drop();
    for _ in 0..600 {
        g.tick(16);
        if g.is_game_over()
            || g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. }))
        {
            return;
        }
    }
}

/// A signature of an arsenal: (token, qty) per slot.
fn arsenal_sig(g: &Game) -> Vec<(i32, u16)> {
    (0..10).map(|s| (g.arsenal_token(s), g.arsenal_quantity(s))).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// SUSAN swaps ARSENALS, not boards. Give A and B distinct arsenals AND
    /// distinct boards, deliver Susan, and assert the ARSENALS exchanged while the
    /// BOARDS are untouched. A mutant `Susan => swap_board_with` swaps the wrong
    /// pair and fails (boards move, arsenals don't).
    #[test]
    fn susan_swaps_arsenals_not_boards(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        a_tokens in prop::collection::vec(0usize..34, 1..5),
        b_tokens in prop::collection::vec(0usize..34, 1..5),
    ) {
        let mut a = Game::new(seed_a);
        let mut b = Game::new(seed_b);
        for &i in &a_tokens { a.grant_weapon(WeaponToken::ALL[i]); }
        for &i in &b_tokens { b.grant_weapon(WeaponToken::ALL[i]); }
        // Give the two boards visibly different fills so a board-swap would show.
        a.board_mut().set(0, 27, Some(Cell::die(3)));
        b.board_mut().set(5, 27, Some(Cell::die(6)));
        b.board_mut().set(6, 27, Some(Cell::die(6)));

        let a_ars0 = arsenal_sig(&a);
        let b_ars0 = arsenal_sig(&b);
        let a_board0 = a.export_board();
        let b_board0 = b.export_board();
        // Precondition: the two arsenals genuinely differ (else the swap is a no-op).
        prop_assume!(a_ars0 != b_ars0);

        deliver_weapon(&mut a, &mut b, WeaponToken::Susan);

        // Arsenals exchanged.
        prop_assert_eq!(arsenal_sig(&a), b_ars0.clone(), "A must now hold B's arsenal");
        prop_assert_eq!(arsenal_sig(&b), a_ars0.clone(), "B must now hold A's arsenal");
        // Boards UNTOUCHED (Susan is arsenal-only).
        prop_assert_eq!(a.export_board(), a_board0, "Susan must NOT swap boards (A board changed)");
        prop_assert_eq!(b.export_board(), b_board0, "Susan must NOT swap boards (B board changed)");
    }

    /// KEATING relayed from A to B: at B's next lock its funds are seized to 0,
    /// and the relay CREDITS that amount to A. Routing it to the wrong side
    /// (crediting B, or zeroing A) fails.
    #[test]
    fn keating_seizes_victim_funds_and_credits_attacker(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        b_funds in 1i64..1_000_000,
    ) {
        let mut v = Versus::new(seed_a, seed_b);
        // Bank some funds on B (the victim) and none on A.
        v.game_mut(Side::B).add_funds(b_funds);
        let a_funds0 = v.game(Side::A).score().funds;
        prop_assert_eq!(v.game(Side::B).score().funds, b_funds);

        // A fires Keating at B (grant + launch), tick to relay/deliver (queue on B).
        v.game_mut(Side::A).grant_weapon(WeaponToken::Keating);
        v.game_mut(Side::A).launch_weapon(0);
        v.tick(16);
        // Flush on B: drive B to a lock so the queued Keating applies.
        for _ in 0..600 {
            v.game_mut(Side::B).begin_drop();
            v.tick(16);
            if v.is_over() || v.game(Side::B).score().funds == 0 { break; }
        }

        prop_assert_eq!(v.game(Side::B).score().funds, 0,
            "Keating must seize ALL of the victim's funds");
        prop_assert_eq!(v.game(Side::A).score().funds, a_funds0 + b_funds,
            "the seized funds must be credited to the attacker (A)");
    }

    /// SCORE/FUNDS relay across sides: whatever score/lines/funds B banks must be
    /// mirrored into A's `op_score`/`op_lines`/`op_funds` (and vice versa) via the
    /// `Scored` event the relay forwards as `receive_op_score`. A mutant that drops
    /// or misroutes the Scored relay leaves the mirror at 0.
    #[test]
    fn score_and_funds_mirror_across_the_relay(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
    ) {
        let mut v = Versus::new(seed_a, seed_b);
        // Make B clear a line so it banks real score/funds: prefill B's bottom rows.
        {
            let b = v.game_mut(Side::B).board_mut();
            let (w, h) = (b.width, b.height);
            for y in [h - 1, h - 2] {
                for x in 0..w { b.set(x, y, Some(Cell::die(6))); }
            }
        }
        // Drive B to a lock (clears those rows -> Scored relayed to A as op_*).
        for _ in 0..600 {
            v.game_mut(Side::B).begin_drop();
            v.tick(16);
            if v.is_over() || v.game(Side::B).score().lines > 0 { break; }
        }
        let b_score = v.game(Side::B).score();
        prop_assume!(b_score.lines > 0); // B actually cleared a line

        // A's mirror of B must match B's real score/lines/funds.
        let a_mirror = v.game(Side::A).score();
        prop_assert_eq!(a_mirror.op_lines, b_score.lines,
            "A.op_lines must mirror B.lines via the relay");
        prop_assert_eq!(a_mirror.op_score, b_score.score,
            "A.op_score must mirror B.score via the relay");
        prop_assert_eq!(a_mirror.op_funds, b_score.funds,
            "A.op_funds must mirror B.funds via the relay");
    }
}

// ---------------------------------------------------------------------------
// (e) MIRROR routing, isolated via deliver_weapon (the relay core).
//   An un-cursed launcher hits the opponent; a Mirror-cursed launcher's offensive
//   weapon BACKFIRES onto itself (nullify-9 fizzle aside). Pins the Mirror branch
//   directly so a mutant that ignores the curse (always hits the opponent) fails.
//   We use RiseUp's UNMISTAKABLE signature — a near-solid bottom row of width-1
//   cells, which no single falling piece can deposit in one row — to tell apart
//   "the bottom row got a RiseUp" from ordinary piece-lock cell growth (which
//   confounds a plain total-cell-count check, since the flushing lock drops a
//   piece either way).
// ---------------------------------------------------------------------------

/// Number of filled cells in the bottom row (the RiseUp signature: width-1).
fn bottom_row_fill(g: &Game) -> i32 {
    let b = g.board();
    let y = b.height - 1;
    (0..b.width).filter(|&x| b.get(x, y).is_some()).count() as i32
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn mirror_curse_backfires_offensive_weapons_onto_the_launcher(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
    ) {
        let mut atk = Game::new(seed_a);
        let mut vic = Game::new(seed_b);
        // Curse the attacker: deliver Mirror onto them and lock to arm it.
        deliver_weapon(&mut vic, &mut atk, WeaponToken::Mirror);
        lock(&mut atk);
        prop_assume!(atk.weapon_active(WeaponToken::Mirror));
        // Both bottom rows start essentially empty (a lone locked piece deposits
        // at most a couple of cells in the bottom row).
        prop_assume!(bottom_row_fill(&atk) < 9 && bottom_row_fill(&vic) < 9);
        let vic_bottom0 = bottom_row_fill(&vic);

        // RiseUp is NOT on the nullify list -> it backfires onto the cursed
        // launcher. Lock the ATTACKER to flush the (backfired) queued RiseUp.
        deliver_weapon(&mut atk, &mut vic, WeaponToken::RiseUp);
        lock(&mut atk);

        // The attacker's bottom row now carries the RiseUp garbage row (>=9 cells),
        // which a single piece-lock can't produce — proving the backfire landed
        // on the LAUNCHER, not the opponent.
        prop_assert!(bottom_row_fill(&atk) >= 9,
            "a cursed launcher's RiseUp must backfire onto its OWN board (bottom row {})",
            bottom_row_fill(&atk));
        // The victim never locked and was never targeted: its bottom row is unchanged.
        prop_assert_eq!(bottom_row_fill(&vic), vic_bottom0,
            "the victim must be spared when the launcher is mirror-cursed");
    }
}
