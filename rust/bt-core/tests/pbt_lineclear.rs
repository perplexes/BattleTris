//! Property-based tests for line-clear correctness.
//!
//!  (a) no completely-filled row ever remains after a lock (during real play),
//!  (b) filled-cell count is conserved across each Locked event,
//!  (c) `check_lines` on a CONSTRUCTED board removes exactly the full rows,
//!      conserves cells, and shifts the survivors down in order.
//!
//! (a)/(b) drive real random play (clears are rare there but the invariants must
//! still hold); (c) forces clears directly so the shift logic is actually
//! exercised. The exact-value behaviour of `check_lines` is additionally pinned
//! against a reference model in `differential_lineclear.rs`.

use bt_core::{Board, Cell, Game, GameEvent};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Left,
    Right,
    Rotate,
    Soft,
    Drop,
    Tick,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => Just(Op::Tick),
        1 => Just(Op::Left),
        1 => Just(Op::Right),
        1 => Just(Op::Rotate),
        1 => Just(Op::Soft),
        1 => Just(Op::Drop),
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
    }
}

fn board_filled_count(g: &Game) -> i64 {
    let b = g.board();
    (0..b.height)
        .flat_map(|y| (0..b.width).map(move |x| (x, y)))
        .filter(|&(x, y)| b.get(x, y).is_some())
        .count() as i64
}

fn piece_cell_count(g: &Game) -> i64 {
    match g.current_piece() {
        Some(p) => p.cells.iter().flatten().filter(|c| c.is_some()).count() as i64,
        None => 0,
    }
}

/// One board row as a bitmask (bit x set = cell occupied).
fn row_mask(b: &Board, y: i32, w: usize) -> u32 {
    let mut m = 0u32;
    for x in 0..w {
        if b.get(x as i32, y).is_some() {
            m |= 1 << x;
        }
    }
    m
}

fn assert_no_full_rows(g: &Game) -> Result<(), TestCaseError> {
    let b = g.board();
    for y in 0..b.height {
        let full = (0..b.width).all(|x| b.get(x, y).is_some());
        prop_assert!(!full, "row {} is fully filled after a lock — line-clear missed it", y);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// (a) No completely-filled row ever remains on the board after a lock.
    #[test]
    fn no_full_row_after_lock(seed in any::<u64>(), ops in prop::collection::vec(op(), 0..256)) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() { break; }
            apply(&mut g, o);
            assert_no_full_rows(&g)?;
        }
    }

    /// (b) CONSERVATION: filled_after == prev_filled + piece_cells - lines*width
    /// on every Locked event, during real play.
    #[test]
    fn conservation_on_lock(seed in any::<u64>(), ops in prop::collection::vec(op(), 0..256)) {
        let mut g = Game::new(seed);
        let width = g.board().width as i64;
        for o in &ops {
            if g.is_game_over() { break; }
            let prev_filled = board_filled_count(&g);
            let piece_cells = piece_cell_count(&g);
            apply(&mut g, o);
            for ev in &g.take_events() {
                if let GameEvent::Locked { lines, .. } = ev {
                    let after = board_filled_count(&g);
                    let expected = prev_filled + piece_cells - (*lines as i64) * width;
                    prop_assert_eq!(after, expected,
                        "conservation violated (lines={}): prev={} piece={} -> {} != {}",
                        lines, prev_filled, piece_cells, expected, after);
                }
            }
        }
    }

    /// (d) GAME-LEVEL FUNDS CREDIT: when `Game::place` clears lines, the player's
    /// `score.funds` must increase by EXACTLY `value * lines` (the funds the clear
    /// is worth, BTBoardManager.C). The board-level differential pins
    /// `check_lines().funds`, but NOT that `place` credits the right field — a
    /// mutant crediting `clear.value` (the raw pip sum) instead of `clear.funds`
    /// (= value*lines) diverges only when lines >= 2. We prefill 2 full die rows so
    /// the next lock clears >= 2 lines with non-zero value, then assert the funds
    /// delta equals the Locked event's `value * lines`.
    #[test]
    fn place_credits_funds_value_times_lines(
        seed in any::<u64>(),
        // pip value 1..=6 so value > 0 (and varies the magnitude).
        pip in 1u8..=6,
    ) {
        let mut g = Game::new(seed);
        // Two full rows of die(pip) on the floor -> the next lock clears both.
        {
            let b = g.board_mut();
            let (w, h) = (b.width, b.height);
            for y in [h - 1, h - 2] {
                for x in 0..w { b.set(x, y, Some(Cell::die(pip))); }
            }
        }
        let funds_before = g.score().funds;

        // Drop + tick until the lock clears the rows; capture the Locked event.
        g.begin_drop();
        let mut cleared: Option<(i32, i32)> = None; // (lines, value)
        for _ in 0..1200 {
            for ev in g.take_events() {
                if let GameEvent::Locked { lines, value, .. } = ev {
                    if lines > 0 { cleared = Some((lines, value)); }
                }
            }
            if cleared.is_some() || g.is_game_over() { break; }
            g.tick(16);
        }
        let Some((lines, value)) = cleared else {
            // The piece may have landed without completing (rare with full
            // pre-rows it shouldn't, but guard against a degenerate spawn).
            return Ok(());
        };
        prop_assert!(lines >= 2, "the two prefilled rows must clear together (got {})", lines);
        prop_assert!(value > 0, "die rows must have non-zero pip value");

        let delta = g.score().funds - funds_before;
        prop_assert_eq!(delta, (value * lines) as i64,
            "place must credit funds = value * lines = {} * {} = {}, got {}",
            value, lines, value * lines, delta);
        // And it must NOT merely credit `value` (the mutant) — non-vacuous because
        // lines >= 2 makes value*lines strictly greater than value.
        prop_assert_ne!(delta, value as i64,
            "funds delta equals the raw value (not value*lines) — wrong credit field");
    }

    /// (c) `check_lines` on a CONSTRUCTED board: removes exactly the full rows,
    /// conserves cells, leaves NO full row, and shifts the survivors (filled AND
    /// empty rows, in order) down so they're bottom-aligned. This forces real
    /// clears that random play almost never produces.
    #[test]
    fn check_lines_clears_full_rows_and_preserves_order(
        seed in any::<u64>(),
        // a random mask per row, plus which rows to force completely full
        masks in prop::collection::vec(0u32..1024, 0..28usize),
        force_full in prop::collection::vec(any::<bool>(), 0..28usize),
    ) {
        let mut g = Game::new(seed);
        let b = g.board_mut();
        b.clear();
        let w = b.width as usize;
        let h = b.height as usize;
        let full = (1u32 << w) - 1;

        // Lay the rows at the BOTTOM of the well (a realistic stack).
        let n = masks.len().min(h);
        for i in 0..n {
            let y = (h - n + i) as i32;
            let mut mask = masks[i] & full;
            if force_full.get(i).copied().unwrap_or(false) {
                mask = full;
            }
            for x in 0..w {
                if mask & (1 << x) != 0 {
                    b.set(x as i32, y, Some(Cell::color(1)));
                }
            }
        }

        let before: Vec<u32> = (0..h as i32).map(|y| row_mask(b, y, w)).collect();
        let before_filled: i64 = before.iter().map(|r| r.count_ones() as i64).sum();
        let n_full = before.iter().filter(|&&r| r == full).count() as i64;

        let _ = b.check_lines();

        let after: Vec<u32> = (0..h as i32).map(|y| row_mask(b, y, w)).collect();
        let after_filled: i64 = after.iter().map(|r| r.count_ones() as i64).sum();

        // No full row survives.
        prop_assert!(after.iter().all(|&r| r != full), "a full row survived check_lines");
        // Cells conserved minus the cleared full rows.
        prop_assert_eq!(after_filled, before_filled - n_full * w as i64, "cell count off");
        // Survivors (non-full rows, top->bottom) end bottom-aligned with empty pad on top.
        let kept: Vec<u32> = before.iter().copied().filter(|&r| r != full).collect();
        let pad = h - kept.len();
        let expected: Vec<u32> = std::iter::repeat(0u32).take(pad).chain(kept).collect();
        prop_assert_eq!(after, expected, "rows did not shift down in order");
    }
}
