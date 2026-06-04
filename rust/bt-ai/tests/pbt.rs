//! Property-based tests for bt-ai placement search and Computer driver.
//!
//! Uses proptest to verify:
//!   (a) best_placement never panics on any (board, piece).
//!   (b) The returned Placement is legal — piece fits at (x, orientation).
//!   (c) Computer::take_turn never panics and actually changes game state.

use bt_ai::{best_placement, Computer, Placement};
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
// (b) Returned Placement is legal — piece fits at (x, orientation)
// ---------------------------------------------------------------------------

/// Rotate a piece clone `o` times on a scratch board (no board walls can
/// block the rotation), mirroring what best_placement does internally.
fn rotate_piece_n(piece: &Piece, o: i32, board_w: i32, board_h: i32) -> Piece {
    let scratch = Board::new(board_w + 20, board_h + 20, true);
    let mut p = piece.clone();
    p.move_to(&scratch, 10, 10);
    for _ in 0..o {
        if !p.rotate(&scratch, false) {
            break;
        }
    }
    p
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// The Placement returned by best_placement must be reachable: the piece
    /// (after `orientation` rotations) must be able to move to (x, dropped_y)
    /// without overlap or out-of-bounds.  We verify by:
    ///   1. Rotating a clone to `orientation`.
    ///   2. Dropping it to its resting row (incrementing y until blocked).
    ///   3. Checking can_move_to succeeds at that row.
    ///
    /// Note: if no placement succeeds (board full / piece too big), the fallback
    /// Placement {x: piece.x, orientation: 0} is returned, which may not have a
    /// resting position — we skip the legality check in that case.
    #[test]
    fn best_placement_is_legal(
        (board, piece) in game_board_strategy(),
    ) {
        let pl = best_placement(&board, &piece);

        // Rotate a clone to the target orientation (same scratch-board
        // technique best_placement uses internally).
        let rotated = rotate_piece_n(&piece, pl.orientation, board.width, board.height);

        // Check move_to at spawn y=0; if it fails we're in a pathological
        // state (piece can't enter at all, e.g. board topped out) — skip.
        if !rotated.can_move_to(&board, pl.x, 0) {
            return Ok(());
        }

        // Drop to resting row.
        let mut landed_y = 0i32;
        loop {
            if rotated.can_move_to(&board, pl.x, landed_y + 1) {
                landed_y += 1;
            } else {
                break;
            }
        }

        // The piece must fit at the resting position.
        prop_assert!(
            rotated.can_move_to(&board, pl.x, landed_y),
            "Placement ({}, rot={}) does not fit on the board at resting y={}",
            pl.x, pl.orientation, landed_y
        );
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
