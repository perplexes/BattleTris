//! Property-based robustness tests for the BattleTris engine.
//!
//! Exercises the full public mutating surface — player inputs, weapons, bazaar,
//! `restore_bytes` with garbage — and asserts three engine invariants:
//!
//!  (a) NO PANIC — any sequence of any ops, including garbage restore_bytes,
//!      must never panic.
//!  (b) NO-OVERLAP — the falling piece's filled cells never coincide with an
//!      occupied board cell.
//!  (c) funds is never negative.

use bt_core::{Game, WeaponToken};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Op set — the full public mutating surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Op {
    // Input
    Left,
    Right,
    Rotate,
    Soft,
    Drop,
    AiDrop,
    Tick,
    // Weapons / bazaar (single-game ops)
    ReceiveWeapon(usize),  // index into WeaponToken::ALL
    GrantWeapon(usize),
    LaunchWeapon(usize),   // arsenal slot 0..10
    BuyWeapon(usize),      // bazaar buy by token index
    SellWeapon(usize),
    LeaveBazaar,
    AddFunds(i64),
    // Serial codec with random (potentially garbage) bytes
    RestoreGarbage(Vec<u8>),
}

fn weapon_idx() -> impl Strategy<Value = usize> {
    0usize..34
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        // Weight ticking heavily so pieces fall/lock and the full game loop runs.
        5 => Just(Op::Tick),
        1 => Just(Op::Left),
        1 => Just(Op::Right),
        1 => Just(Op::Rotate),
        1 => Just(Op::Soft),
        1 => Just(Op::Drop),
        1 => Just(Op::AiDrop),
        1 => weapon_idx().prop_map(Op::ReceiveWeapon),
        1 => weapon_idx().prop_map(Op::GrantWeapon),
        1 => (0usize..10).prop_map(Op::LaunchWeapon),
        1 => weapon_idx().prop_map(Op::BuyWeapon),
        1 => weapon_idx().prop_map(Op::SellWeapon),
        1 => Just(Op::LeaveBazaar),
        1 => (-500i64..=500i64).prop_map(Op::AddFunds),
        // Garbage restore: a variety of lengths, including multiples of 8 (which
        // pass the length check and hit deeper validation) and non-multiples.
        1 => prop::collection::vec(any::<u8>(), 0..128).prop_map(Op::RestoreGarbage),
    ]
}

fn apply(g: &mut Game, op: &Op) {
    match op {
        Op::Left => g.move_left(),
        Op::Right => g.move_right(),
        Op::Rotate => g.rotate(),
        Op::Soft => g.soft_drop(),
        Op::Drop => g.begin_drop(),
        Op::AiDrop => g.ai_begin_drop(),
        Op::Tick => g.tick(16),
        Op::ReceiveWeapon(i) => g.receive_weapon(WeaponToken::ALL[*i]),
        Op::GrantWeapon(i) => { g.grant_weapon(WeaponToken::ALL[*i]); }
        Op::LaunchWeapon(slot) => g.launch_weapon(*slot),
        Op::BuyWeapon(i) => { g.buy_weapon(WeaponToken::ALL[*i]); }
        Op::SellWeapon(i) => { g.sell_weapon(WeaponToken::ALL[*i]); }
        Op::LeaveBazaar => g.leave_bazaar(),
        Op::AddFunds(amount) => g.add_funds(*amount),
        Op::RestoreGarbage(bytes) => {
            // Must return false (never panic); ignore the result.
            let _ = g.restore_bytes(bytes);
        }
    }
    // Drain events so the queue doesn't grow without bound.
    let _ = g.take_events();
}

// ---------------------------------------------------------------------------
// Helper: check piece/board overlap for the game's current state.
// Returns Some((piece_gx, piece_gy)) if any piece cell coincides with a
// filled board cell, or None if clean.
// ---------------------------------------------------------------------------

fn first_overlap(g: &Game) -> Option<(i32, i32)> {
    use bt_core::constants::{BT_PIECE_WIDTH, BT_PIECE_HEIGHT};
    if let Some(p) = g.current_piece() {
        for i in 0..BT_PIECE_WIDTH {
            for j in 0..BT_PIECE_HEIGHT {
                if p.cells[i][j].is_some() {
                    let gx = p.x + i as i32;
                    let gy = p.y + j as i32;
                    // Board::get returns None for out-of-bounds, so only report
                    // overlaps that are inside the playfield.
                    if g.board().get(gx, gy).is_some() {
                        return Some((gx, gy));
                    }
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// (a) NO PANIC + (b) NO-OVERLAP + (c) funds >= 0
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn no_panic_no_overlap_no_negative_funds(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
    ) {
        let mut g = Game::new(seed);
        // Funds may legitimately go negative only AFTER a Reagan activation or a
        // negative AddFunds. Until one of those is applied, funds must stay >= 0,
        // and we assert that on EVERY step (previously this was a no-op branch and
        // the final check was skipped for the whole run on any Reagan).
        let mut funds_may_be_negative = false;

        for o in &ops {
            // (a) NO PANIC — proptest reports any Rust panic as a failure.
            apply(&mut g, o);

            if matches!(o, Op::ReceiveWeapon(i) if WeaponToken::ALL[*i] == WeaponToken::Reagan)
                || matches!(o, Op::AddFunds(n) if *n < 0)
            {
                funds_may_be_negative = true;
            }

            if g.is_game_over() {
                continue;
            }

            // (b) NO-OVERLAP: the falling piece never coincides with a board cell.
            if let Some((gx, gy)) = first_overlap(&g) {
                prop_assert!(false, "piece/board overlap at ({}, {}) after {:?}", gx, gy, o);
            }

            // (c) funds >= 0 on every step until a Reagan/negative-AddFunds makes
            // a negative balance legitimate.
            if !funds_may_be_negative {
                prop_assert!(
                    g.score().funds >= 0,
                    "funds went negative ({}) after {:?} before any Reagan/negative-AddFunds",
                    g.score().funds, o
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Focused: garbage restore_bytes never panics (covers the no-panic property
// specifically for the codec path with lengths that are multiples of 8).
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn garbage_restore_never_panics(
        seed in any::<u64>(),
        // Multiples of 8 (most interesting: they pass the length check)
        word_count in 0usize..128,
        raw in prop::collection::vec(any::<i64>(), 0usize..128),
    ) {
        let mut g = Game::new(seed);
        // Convert i64 words to LE bytes (multiples-of-8 path).
        let bytes_mul8: Vec<u8> = raw[..word_count.min(raw.len())]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let _ = g.restore_bytes(&bytes_mul8);
        // Game must still be operable.
        g.leave_bazaar();
        g.tick(16);

        // Also try a raw byte vector of arbitrary length.
        let bytes_arb: Vec<u8> = raw.iter().map(|v| *v as u8).collect();
        let _ = g.restore_bytes(&bytes_arb);
        g.tick(16);
    }
}

// ---------------------------------------------------------------------------
// Focused: the falling piece NEVER overlaps the board (higher case count,
// no weapon noise — pure movement/gravity).
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn piece_never_overlaps_board(
        seed in any::<u64>(),
        ops in prop::collection::vec(
            prop_oneof![
                4 => Just(Op::Tick),
                1 => Just(Op::Left),
                1 => Just(Op::Right),
                1 => Just(Op::Rotate),
                1 => Just(Op::Soft),
                1 => Just(Op::Drop),
            ],
            0..256,
        ),
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() { break; }
            apply(&mut g, o);
            if let Some((gx, gy)) = first_overlap(&g) {
                prop_assert!(
                    false,
                    "piece/board overlap at ({}, {}) after {:?}", gx, gy, o
                );
            }
        }
    }
}
