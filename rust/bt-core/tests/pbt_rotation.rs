//! Property-based tests for piece rotation.
//!
//!  (a) rotate preserves the piece's filled-cell count,
//!  (b) after any rotate the active piece's cells are in-bounds and never
//!      overlap a locked cell,
//!  (c) the falling piece never overlaps locked cells,
//!  (d) rotate-then-reverse restores the original cell set when both succeed,
//!  (e) DETERMINISTIC coverage: the special custom-rotation pieces
//!      (Wall / Star / WeirdLong) are actually reached and rotate validly.
//!
//! Random play from a fresh Game never spawns the weird pieces, so the random
//! properties grant `FearedWeird` (which unlocks them) and (e) confirms they're
//! really exercised.

use bt_core::{Board, Game, Piece, PieceKind, WeaponToken};
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

/// A game whose stream includes the weird pieces (Wall/Star/WeirdLong etc.).
fn weird_game(seed: u64) -> Game {
    let mut g = Game::new(seed);
    g.receive_weapon(WeaponToken::FearedWeird);
    g
}

fn count_piece_cells_in(p: &Piece) -> usize {
    p.cells.iter().flatten().filter(|c| c.is_some()).count()
}

fn count_piece_cells(g: &Game) -> usize {
    g.current_piece().map(count_piece_cells_in).unwrap_or(0)
}

fn piece_overlaps_board(g: &Game) -> bool {
    let b = g.board();
    if let Some(p) = g.current_piece() {
        for i in 0..8usize {
            for j in 0..8usize {
                if p.cells[i][j].is_some() && b.get(p.x + i as i32, p.y + j as i32).is_some() {
                    return true;
                }
            }
        }
    }
    false
}

fn piece_out_of_bounds(g: &Game) -> bool {
    let b = g.board();
    if let Some(p) = g.current_piece() {
        for i in 0..8usize {
            for j in 0..8usize {
                if p.cells[i][j].is_some() {
                    let (bx, by) = (p.x + i as i32, p.y + j as i32);
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

    /// (a) Rotate preserves the piece's filled-cell count (while the same piece
    /// is alive).
    #[test]
    fn rotate_preserves_cell_count(seed in any::<u64>(), ops in prop::collection::vec(op(), 0..256)) {
        let mut g = weird_game(seed);
        for o in &ops {
            if g.is_game_over() { break; }
            let before_count = count_piece_cells(&g);
            let before_kind = g.current_piece().map(|p| p.kind);
            apply(&mut g, o);
            let after_kind = g.current_piece().map(|p| p.kind);
            if before_count > 0 && before_kind == after_kind {
                prop_assert_eq!(count_piece_cells(&g), before_count,
                    "piece cell count changed after {:?}", o);
            }
        }
    }

    /// (a') Every live piece always has > 0 cells.
    #[test]
    fn every_piece_always_has_positive_cell_count(seed in any::<u64>(), ops in prop::collection::vec(op(), 0..256)) {
        let mut g = weird_game(seed);
        for o in &ops {
            if g.is_game_over() { break; }
            apply(&mut g, o);
            if let Some(p) = g.current_piece() {
                prop_assert!(count_piece_cells_in(p) > 0, "piece {:?} has zero cells", p.kind);
            }
        }
    }

    /// (b) After any rotate the piece is in-bounds and overlaps no locked cell.
    #[test]
    fn rotate_stays_in_bounds_no_overlap(seed in any::<u64>(), ops in prop::collection::vec(op(), 0..256)) {
        let mut g = weird_game(seed);
        for o in &ops {
            if g.is_game_over() { break; }
            apply(&mut g, o);
            prop_assert!(!piece_out_of_bounds(&g), "piece out of bounds after {:?}", o);
            prop_assert!(!piece_overlaps_board(&g), "piece overlaps locked cell after {:?}", o);
        }
    }

    /// (c) The falling piece never overlaps locked cells.
    #[test]
    fn piece_never_overlaps_locked_cells(seed in any::<u64>(), ops in prop::collection::vec(op(), 0..256)) {
        let mut g = weird_game(seed);
        for o in &ops {
            if g.is_game_over() { break; }
            apply(&mut g, o);
            prop_assert!(!piece_overlaps_board(&g), "falling piece overlaps locked cell after {:?}", o);
        }
    }

    /// (d) rotate(fwd) then rotate(rev) restores the original cells when both succeed.
    #[test]
    fn rotate_reverse_roundtrip(seed in any::<u64>(), ops in prop::collection::vec(op(), 0..256)) {
        let mut g = weird_game(seed);
        for o in &ops {
            if g.is_game_over() { break; }
            if let Some(p) = g.current_piece() {
                if p.rot > 0 {
                    let board = g.board();
                    let cells_before = p.cells;
                    let mut pc = p.clone();
                    if pc.rotate(board, false) && pc.rotate(board, true) {
                        prop_assert_eq!(pc.cells, cells_before,
                            "rotate fwd+rev didn't restore cells for {:?} (rot={}, orient={}, state={})",
                            p.kind, p.rot, p.orientations, p.state);
                    }
                }
            }
            apply(&mut g, o);
        }
    }
}

/// (e) Coverage: with FearedWeird active, the special custom-rotation pieces are
/// actually reached, and rotating them keeps the cell count + leaves the piece
/// in-bounds with no overlap. Deterministic (fixed seed, bounded spawns).
#[test]
fn special_piece_rotation_is_covered_and_valid() {
    // Vary the seed per game — resetting to a fixed seed would just replay the
    // same deterministic piece sequence and never cover all the weird kinds.
    let mut game_no = 0u64;
    let mut g = weird_game(game_no);
    let mut seen: Vec<PieceKind> = Vec::new();
    // Special pieces whose custom rotation actually CHANGED the cells at least
    // once (so a no-op rotation that just returned false can't pass the test).
    let mut rot_changes: Vec<PieceKind> = Vec::new();

    for _ in 0..8000 {
        if g.is_game_over() {
            game_no += 1;
            g = weird_game(game_no);
        }
        // Keep the weird stream lit (flushed each iteration by the drop below).
        g.receive_weapon(WeaponToken::FearedWeird);

        if let Some(p) = g.current_piece() {
            let kind = p.kind;
            if !seen.contains(&kind) {
                seen.push(kind);
            }
            // Confirm the custom rotation for the special pieces is NON-TRIVIAL:
            // rotating a clone on an EMPTY board must actually change its cells.
            if matches!(kind, PieceKind::Wall | PieceKind::Star | PieceKind::WeirdLong)
                && !rot_changes.contains(&kind)
            {
                let empty = Board::standard(false);
                let mut pc = p.clone();
                pc.move_to(&empty, 8, 8);
                let before = pc.cells;
                if pc.rotate(&empty, false) && pc.cells != before {
                    rot_changes.push(kind);
                }
            }
            let base = count_piece_cells(&g);
            // Rotate through its orientations; invariants must hold each time.
            for _ in 0..8 {
                g.rotate();
                assert!(!piece_out_of_bounds(&g), "{:?} rotated out of bounds", kind);
                assert!(!piece_overlaps_board(&g), "{:?} rotated into a locked cell", kind);
                if let Some(pp) = g.current_piece() {
                    if pp.kind == kind {
                        assert_eq!(count_piece_cells_in(pp), base,
                            "{:?} changed cell count under rotation", kind);
                    }
                }
            }
        }
        // Lock the piece and let the next spawn.
        g.begin_drop();
        for _ in 0..50 {
            g.tick(16);
        }
        let _ = g.take_events();
    }

    for k in [PieceKind::Wall, PieceKind::Star, PieceKind::WeirdLong] {
        assert!(seen.contains(&k), "coverage gap: never spawned {:?} (custom rotation untested)", k);
        assert!(rot_changes.contains(&k),
            "{:?} rotation never changed its cells — custom rotation may be a no-op", k);
    }
}
