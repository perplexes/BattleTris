//! Differential test for line-clearing + gravity — the most fundamental, and
//! most off-by-one-prone, game rule.
//!
//! `Board::check_lines` is the faithful analogue of `BTBoardManager::checkLines`
//! + `removeLine` (BTBoardManager.C:551-617, 311-...). Real FFI against the 1994
//! C++ is impractical — those functions are tangled with X11/Motif, packet
//! `send()`s and display redraws. So instead this file carries an INDEPENDENT,
//! deliberately naive reference implementation (operating on a plain
//! `Vec<Vec<Option<i32>>>` grid, no Cell/weapon/idiot machinery) and fuzzes the
//! two against each other over thousands of random boards. Divergence in the
//! cleared counts, the funds, OR the resulting grid fails the test.
//!
//! Scope: the no-weapon path (Force/Bottle/Upbyside off, human board). Those
//! variants get their own targeted tests; here we pin the core clear+cascade.
//! Cells are plain colors (value 0) and dice (value 1..=6) — no happy faces, so
//! the "landed → frown" mutation never fires and a grid of `i32` values is a
//! faithful mirror of the engine's `Cell` grid.

use bt_core::constants::{BT_BOARD_HGT, BT_BOARD_WTH};
use bt_core::rng::Rng;
use bt_core::{Board, Cell};

/// Independent reference: clear full rows bottom-up, cascading everything above
/// a cleared row down by one (standard gravity), accumulating `value` and
/// returning `(lines, value, funds = value * lines)`. Mirrors the engine's
/// loop structure (re-examine a cleared row after the shift) without sharing
/// any of its code.
fn ref_check_lines(rows: &mut [Vec<Option<i32>>]) -> (i32, i32, i32) {
    let h = rows.len() as i32;
    let w = rows[0].len();
    let mut value = 0i32;
    let mut lines = 0i32;

    let mut j = h - 1;
    while j >= 0 {
        let jy = j as usize;
        let full = (0..w).all(|x| rows[jy][x].is_some());
        if full {
            lines += 1;
            value += (0..w).map(|x| rows[jy][x].unwrap()).sum::<i32>();
            // removeLine(j): shift rows [0..j] down into j, then empty row 0.
            let mut i = j;
            while i > 0 {
                rows[i as usize] = rows[(i - 1) as usize].clone();
                i -= 1;
            }
            rows[0] = vec![None; w];
            // Re-examine row j — the board shifted down into it. (No j--.)
        } else {
            j -= 1;
        }
    }
    (lines, value, value * lines)
}

/// Snapshot the engine board as a plain value grid (`None` = empty).
fn engine_grid(b: &Board) -> Vec<Vec<Option<i32>>> {
    (0..b.height)
        .map(|y| (0..b.width).map(|x| b.get(x, y).map(|c| c.value())).collect())
        .collect()
}

#[test]
fn line_clear_matches_independent_reference() {
    let mut rng = Rng::new(0xBADC0FFEE);
    let w = BT_BOARD_WTH;
    let h = BT_BOARD_HGT;
    const ITERS: usize = 4_000;

    // Coverage guards so the funds/value comparison can't go vacuous: `funds`
    // only distinguishes `value * lines` from `value` when lines >= 2, and the
    // value sum is only meaningful when a non-empty clear actually happens.
    let mut saw_single_clear = 0usize;
    let mut saw_multi_clear = 0usize;
    let mut saw_multi_nonzero_value = 0usize;

    for iter in 0..ITERS {
        // Diverse fill densities: dense boards exercise multi-line cascades,
        // sparse boards exercise holes / partial rows / the no-clear path.
        let fill_pct = 45 + rng.rand_below(50); // 45..=94

        let mut board = Board::standard(false);
        let mut rows: Vec<Vec<Option<i32>>> = vec![vec![None; w as usize]; h as usize];

        for y in 0..h {
            for x in 0..w {
                if rng.rand_below(100) < fill_pct {
                    let v = rng.rand_below(7); // 0..=6
                    let cell = if v == 0 {
                        Cell::color(1 + rng.rand_below(5)) // value 0
                    } else {
                        Cell::die(v as u8) // value v
                    };
                    debug_assert_eq!(cell.value(), v);
                    board.set(x, y, Some(cell));
                    rows[y as usize][x as usize] = Some(v);
                }
            }
        }

        let before = engine_grid(&board);

        let lc = board.check_lines();
        let (rl, rv, rf) = ref_check_lines(&mut rows);

        match lc.lines {
            1 => saw_single_clear += 1,
            n if n >= 2 => {
                saw_multi_clear += 1;
                // funds = value * lines diverges from `value` only when BOTH
                // lines >= 2 AND value > 0; pin that exact case was exercised.
                if lc.value > 0 {
                    saw_multi_nonzero_value += 1;
                }
            }
            _ => {}
        }

        assert_eq!(
            (lc.lines, lc.value, lc.funds),
            (rl, rv, rf),
            "iter {iter} (fill {fill_pct}%): engine {:?} vs reference (lines {rl}, value {rv}, funds {rf})\nboard before:\n{}",
            (lc.lines, lc.value, lc.funds),
            dump(&before)
        );

        let after = engine_grid(&board);
        assert_eq!(
            after, rows,
            "iter {iter} (fill {fill_pct}%): post-clear grid diverged\nbefore:\n{}\nengine after:\n{}\nreference after:\n{}",
            dump(&before),
            dump(&after),
            dump(&rows)
        );
    }

    // Non-vacuity: the run must have exercised both a single-line clear AND a
    // multi-line clear, so `funds = value * lines` is genuinely distinguished
    // from `funds = value` (they agree only at lines == 1).
    assert!(
        saw_single_clear > 0 && saw_multi_clear > 0 && saw_multi_nonzero_value > 0,
        "line-clear coverage too thin (single={saw_single_clear}, multi={saw_multi_clear}, \
         multi_nonzero_value={saw_multi_nonzero_value}); the value/funds comparison may be vacuous"
    );
}

/// Compact board dump for failure messages: '.' empty, digit = cell value.
fn dump(rows: &[Vec<Option<i32>>]) -> String {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|c| match c {
                    None => '.',
                    Some(v) => std::char::from_digit(*v as u32, 10).unwrap_or('#'),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
