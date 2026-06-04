//! Property-based tests for bt-ai placement search and Computer driver.
//!
//! Uses proptest to verify:
//!   (a) best_placement never panics on any (board, piece).
//!   (b) The returned Placement is legal — piece fits at (x, orientation).
//!   (c) Computer::take_turn never panics and actually changes game state.

use bt_ai::{best_placement, eval_board, Computer, Placement};
use bt_core::{Board, Game, Piece, PieceKind};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// All standard piece kinds.
fn piece_kind() -> impl Strategy<Value = PieceKind> {
    prop_oneof![
        Just(PieceKind::El),
        Just(PieceKind::RevEl),
        Just(PieceKind::SlideRight),
        Just(PieceKind::SlideLeft),
        Just(PieceKind::Long),
        Just(PieceKind::Plug),
        Just(PieceKind::Box),
        Just(PieceKind::Die),
        Just(PieceKind::Happy),
        Just(PieceKind::Dog),
        Just(PieceKind::RevDog),
        Just(PieceKind::Cap),
        Just(PieceKind::Wall),
        Just(PieceKind::Tower),
        Just(PieceKind::Star),
        Just(PieceKind::WeirdLong),
        Just(PieceKind::FourByFour),
        Just(PieceKind::LongDong),
    ]
}

/// Helper: derive a random board state by playing a Game with a random seed
/// for a random number of ticks then extracting its board.  We use the tick
/// count to spread across early/mid/late board states.
fn game_board_strategy() -> impl Strategy<Value = (Board, Piece)> {
    (any::<u64>(), piece_kind(), 1u8..4u8, 0u64..2000u64).prop_map(
        |(seed, kind, die_value, ticks)| {
            let mut g = Game::new(seed);
            // Advance the game by applying random ticks to get a non-trivial
            // board state, stopping at game over so we have a valid board.
            for _ in 0..ticks {
                if g.is_game_over() {
                    break;
                }
                g.tick(16);
            }
            let board = g.board().clone();
            // Spawn the piece at the standard centre spawn (x=5, y=0).
            let piece = Piece::construct(kind, 5, 0, die_value.max(1));
            (board, piece)
        },
    )
}

// ---------------------------------------------------------------------------
// (a) best_placement never panics
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// best_placement must not panic for any reachable (board, piece) pair.
    #[test]
    fn best_placement_never_panics(
        (board, piece) in game_board_strategy(),
    ) {
        // The test passes if this doesn't panic.
        let _placement: Placement = best_placement(&board, &piece);
    }
}

// ---------------------------------------------------------------------------
// (b) best_placement returns an IN-RANGE, ENTERABLE placement.
//
// best_placement only ever CONSIDERS a candidate (x, o) where the freely-rotated
// piece can move_to(board, x, 0) — i.e. enter at the top of the well (see its
// source). So its OUTPUT must satisfy that same invariant. We verify it
// independently: rotate a clone to pl.orientation (free, on a scratch board) and
// confirm it can enter at column pl.x. If ANY legal placement exists, a returned
// placement that can't even enter (a wrong column/orientation) is a real bug —
// which the previous "drive the engine and see if anything locks" check missed
// (an absurd column just walked into the wall and dropped wherever).
// ---------------------------------------------------------------------------

/// Rotate a clone `o` times on a large scratch board (free rotation), exactly as
/// best_placement does to derive a rotated shape.
fn rotated_clone(piece: &Piece, o: i32, board: &Board) -> Piece {
    let scratch = Board::new(board.width + 20, board.height + 20, true);
    let mut p = piece.clone();
    p.move_to(&scratch, 10, 10);
    for _ in 0..o {
        if !p.rotate(&scratch, false) {
            break;
        }
    }
    p
}

/// Does ANY (orientation, column) let the piece enter at the top of `board`?
fn any_enterable(board: &Board, piece: &Piece) -> bool {
    let xmin = -(bt_core::constants::BT_PIECE_WIDTH as i32 - 1);
    for o in 0..piece.orientations.max(1) {
        let rp = rotated_clone(piece, o, board);
        for x in xmin..board.width {
            if rp.can_move_to(board, x, 0) {
                return true;
            }
        }
    }
    false
}

/// Replicate best_placement's per-candidate simulation for one (column,
/// orientation): rotate the piece `o` times (free, on a scratch board), enter at
/// the spawn row, hard-drop, land into a board clone, clear lines, then
/// `eval_board`. Returns `None` if the piece can't enter at column `x` (exactly
/// the candidates best_placement `continue`s past). Uses only public APIs +
/// the public `eval_board`, so the SELECTION logic is recomputed independently.
fn simulate_and_eval(board: &Board, piece: &Piece, x: i32, o: i32) -> Option<f64> {
    let mut b = board.clone();
    let mut p = rotated_clone(piece, o, board);
    if !p.move_to(&b, x, 0) {
        return None;
    }
    let mut landed_y = 0;
    while p.can_move_to(&b, x, landed_y + 1) {
        landed_y += 1;
    }
    if !p.move_to(&b, x, landed_y) {
        return None;
    }
    p.land(&mut b);
    b.check_lines();
    Some(eval_board(&b))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    // -----------------------------------------------------------------------
    // best_placement must return the GLOBAL-MINIMUM eval score, not merely a
    // legal placement. A "pick the first legal candidate" mutant (e.g. changing
    // the selection guard `if score < best_score` to `if best_score == f64::MAX`)
    // keeps every returned placement legal/enterable — so the enterability test
    // below can't catch it. Here we recompute, over the SAME candidate set and
    // the SAME land+clear+eval simulation, the minimum achievable score and
    // assert best_placement actually attains it.
    // -----------------------------------------------------------------------
    #[test]
    fn best_placement_attains_global_minimum_score(
        (board, piece) in game_board_strategy(),
    ) {
        // Only meaningful when at least one placement is possible.
        if !any_enterable(&board, &piece) {
            return Ok(());
        }

        let pl = best_placement(&board, &piece);
        let chosen = simulate_and_eval(&board, &piece, pl.x, pl.orientation)
            .expect("best_placement returned a placement that can't even enter");

        // Global minimum over every (orientation, column) in best_placement's
        // search range. Order is irrelevant for the minimum.
        let xmin = -(bt_core::constants::BT_PIECE_WIDTH as i32 - 1);
        let mut global_min = f64::MAX;
        for o in 0..piece.orientations.max(1) {
            for x in xmin..board.width {
                if let Some(s) = simulate_and_eval(&board, &piece, x, o) {
                    if s < global_min {
                        global_min = s;
                    }
                }
            }
        }

        prop_assert!(
            (chosen - global_min).abs() < 1e-9,
            "best_placement is sub-optimal: chose score {} at (x={}, o={}) but the \
             global minimum over all candidates is {}",
            chosen, pl.x, pl.orientation, global_min
        );
    }

    #[test]
    fn best_placement_is_in_range_and_enterable(
        (board, piece) in game_board_strategy(),
    ) {
        let pl = best_placement(&board, &piece);

        // Orientation in range.
        prop_assert!(pl.orientation >= 0 && pl.orientation < piece.orientations.max(1),
            "orientation {} out of range (orientations={})", pl.orientation, piece.orientations);

        // If any legal placement exists, the returned one must be enterable.
        if any_enterable(&board, &piece) {
            let rp = rotated_clone(&piece, pl.orientation, &board);
            prop_assert!(rp.can_move_to(&board, pl.x, 0),
                "best_placement returned a NON-ENTERABLE placement (x={}, o={}) although legal ones exist",
                pl.x, pl.orientation);
        }
    }
}

// ---------------------------------------------------------------------------
// (c) Computer::take_turn never panics and advances game state
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Computer::take_turn must not panic and must change the game's
    /// observable state (the board or the current piece changes, or the game
    /// ends — all of those count as "the AI did something").
    #[test]
    fn take_turn_never_panics_and_advances(
        seed in any::<u64>(),
        // How many ticks to apply before handing off to the AI (spreads
        // across empty / partially-filled board states).
        pre_ticks in 0u64..500u64,
    ) {
        let mut g = Game::new(seed);
        for _ in 0..pre_ticks {
            if g.is_game_over() {
                break;
            }
            g.tick(16);
        }

        if g.is_game_over() {
            // Nothing to assert when the game is already over.
            return Ok(());
        }

        // Snapshot state before take_turn.
        let board_before = g.board().clone();
        let piece_x_before = g.current_piece().map(|p| p.x);
        let piece_orient_before = g.current_piece().map(|p| p.orientation);
        let score_before = g.score();

        let mut ernie = Computer::new();
        // This must not panic.
        ernie.take_turn(&mut g);

        // Tick a bit so the piece actually falls and locks (take_turn calls
        // ai_begin_drop but the engine needs ticks to process it).
        for _ in 0..200 {
            if g.is_game_over() {
                break;
            }
            g.tick(16);
            g.take_events();
        }

        // State must have changed: either the piece moved, a new piece spawned
        // (board changed), or the game ended.
        let board_after = g.board().clone();
        let piece_x_after = g.current_piece().map(|p| p.x);
        let piece_orient_after = g.current_piece().map(|p| p.orientation);
        let score_after = g.score();

        let board_changed = board_before.width != board_after.width
            || (0..board_before.height).any(|y| {
                (0..board_before.width).any(|x| board_before.occupied(x, y) != board_after.occupied(x, y))
            });
        let piece_changed = piece_x_after != piece_x_before
            || piece_orient_after != piece_orient_before;
        let score_changed = score_after.score != score_before.score
            || score_after.lines != score_before.lines;
        let game_over = g.is_game_over();

        prop_assert!(
            board_changed || piece_changed || score_changed || game_over,
            "take_turn + ticks left game completely unchanged \
             (pre_ticks={}, piece_x={:?}->{:?}, orient={:?}->{:?})",
            pre_ticks,
            piece_x_before, piece_x_after,
            piece_orient_before, piece_orient_after,
        );
    }
}
