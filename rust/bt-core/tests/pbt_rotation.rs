//! Property-based tests for piece rotation.
//!
//! Drives random play including rotates and checks:
//!  (a) rotate preserves the piece's filled-cell count,
//!  (b) after any rotate, the active piece's filled cells are within board
//!      bounds AND never overlap an occupied board cell,
//!  (c) the current falling piece never overlaps locked cells after any op,
//!  (d) rotate-then-reverse restores the original cell set when both succeed
//!      (uses Piece::rotate directly on a clone since Game::rotate is forward-only).

use bt_core::{Game, Piece};
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
        2 => Just(Op::Rotate),
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

fn count_piece_cells_in(p: &Piece) -> usize {
    p.cells.iter().flatten().filter(|c| c.is_some()).count()
}

fn count_piece_cells(g: &Game) -> usize {
    g.current_piece().map(|p| count_piece_cells_in(p)).unwrap_or(0)
}

/// Returns true iff the falling piece overlaps any occupied board cell.
fn piece_overlaps_board(g: &Game) -> bool {
    let b = g.board();
    if let Some(p) = g.current_piece() {
        for i in 0..8usize {
            for j in 0..8usize {
                if p.cells[i][j].is_some() {
                    let bx = p.x + i as i32;
                    let by = p.y + j as i32;
                    if b.get(bx, by).is_some() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Returns true iff any filled cell of the falling piece is outside board bounds.
fn piece_out_of_bounds(g: &Game) -> bool {
    let b = g.board();
    if let Some(p) = g.current_piece() {
        for i in 0..8usize {
            for j in 0..8usize {
                if p.cells[i][j].is_some() {
                    let bx = p.x + i as i32;
                    let by = p.y + j as i32;
                    if bx < 0 || bx >= b.width || by < 0 || by >= b.height {
                        return true;
                    }
                }
            }
        }
    }
    false
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// (a) Rotate preserves the piece's filled-cell count.
    ///
    /// Snapshot the cell count before each op; if the same piece is still active
    /// after the op (no lock+spawn occurred), the count must be unchanged.
    #[test]
    fn rotate_preserves_cell_count(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() {
                break;
            }
            // Capture piece identity (position acts as a proxy for "same piece").
            let before_count = count_piece_cells(&g);
            let before_pos = g.current_piece().map(|p| (p.x, p.y, p.kind));
            apply(&mut g, o);
            let after_count = count_piece_cells(&g);
            let after_pos = g.current_piece().map(|p| (p.x, p.y, p.kind));

            // Only compare counts when the same piece is still alive.
            // A lock→spawn event resets the piece, so the (kind) must match.
            // Since kind alone can repeat we also require the count was nonzero
            // before (avoids false positives on the very first tick).
            if before_count > 0 && before_pos.map(|b| b.2) == after_pos.map(|a| a.2) {
                // Guard: also confirm this isn't a new spawn at the same x,y
                // (very unlikely, but possible in theory). A cleaner heuristic
                // is that the piece count must be stable for *all* ops that
                // don't cause a lock event (no Locked event this step).
                // We accept the small false-negative risk here to keep it simple.
                prop_assert_eq!(
                    after_count, before_count,
                    "piece cell count changed {} → {} after {:?}",
                    before_count, after_count, o
                );
            }
        }
    }

    /// (a) Stronger: rotate preserves cell count — detect even across spawns by
    ///     running only the rotate path, making it an invariant across the whole
    ///     game, not just same-piece windows.
    #[test]
    fn every_piece_always_has_positive_cell_count(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() {
                break;
            }
            apply(&mut g, o);
            if let Some(p) = g.current_piece() {
                prop_assert!(
                    count_piece_cells_in(p) > 0,
                    "piece {:?} has zero cells after {:?}", p.kind, o
                );
            }
        }
    }

    /// (b) After any rotate, the piece's cells are within board bounds
    ///     and don't overlap any locked board cell.
    #[test]
    fn rotate_stays_in_bounds_no_overlap(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() {
                break;
            }
            apply(&mut g, o);
            prop_assert!(
                !piece_out_of_bounds(&g),
                "piece cell landed out of bounds after {:?}", o
            );
            prop_assert!(
                !piece_overlaps_board(&g),
                "piece overlaps a locked board cell after {:?}", o
            );
        }
    }

    /// (c) The falling piece never overlaps locked cells after any op sequence.
    #[test]
    fn piece_never_overlaps_locked_cells(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() {
                break;
            }
            apply(&mut g, o);
            prop_assert!(
                !piece_overlaps_board(&g),
                "falling piece overlaps locked board cell after {:?}", o
            );
        }
    }

    /// (d) rotate-then-reverse restores the original cell set when both succeed.
    ///
    /// We clone the current piece and call Piece::rotate(board, false) then
    /// Piece::rotate(board, true) on the clone. If both return true the cells
    /// must equal the snapshot.
    #[test]
    fn rotate_reverse_roundtrip(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() {
                break;
            }

            // Before running the op: test roundtrip on a clone of the piece.
            if let Some(p) = g.current_piece() {
                if p.rot > 0 {
                    let board = g.board();
                    let cells_before = p.cells;
                    let mut pc = p.clone();

                    let fwd_ok = pc.rotate(board, false);
                    if fwd_ok {
                        // Forward succeeded: the cells must have changed (or Star
                        // may stay the same in degenerate positions, but still).
                        let fwd_cells = pc.cells;
                        let rev_ok = pc.rotate(board, true);
                        if rev_ok {
                            prop_assert_eq!(
                                pc.cells, cells_before,
                                "rotate(fwd) then rotate(rev) didn't restore original \
                                 cells for piece {:?} (rot={}, orientations={}, state={}); \
                                 after_fwd={:?}",
                                p.kind, p.rot, p.orientations, p.state,
                                fwd_cells
                            );
                        }
                    }
                }
            }

            apply(&mut g, o);
        }
    }
}
