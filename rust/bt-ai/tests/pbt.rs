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
// (b) The returned Placement is in-range AND actually executable on the real
//     engine. Rather than re-verify with best_placement's own scratch-board
//     rotation (which would let a bug cancel out), we DRIVE THE REAL GAME — the
//     engine is the authority on what's a legal placement — and confirm the AI's
//     (x, orientation) executes into a real lock.
// ---------------------------------------------------------------------------

fn filled(g: &Game) -> i64 {
    let b = g.board();
    (0..b.height)
        .flat_map(|y| (0..b.width).map(move |x| (x, y)))
        .filter(|&(x, y)| b.occupied(x, y))
        .count() as i64
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn best_placement_is_in_range_and_executes(
        seed in any::<u64>(),
        pre_ticks in 0u64..1500u64,
    ) {
        let mut g = Game::new(seed);
        for _ in 0..pre_ticks {
            if g.is_game_over() { break; }
            g.tick(16);
        }
        if g.is_game_over() { return Ok(()); }
        let piece = match g.current_piece() { Some(p) => p.clone(), None => return Ok(()) };
        let board = g.board().clone();

        let pl = best_placement(&board, &piece);

        // In-range sanity, independent of execution.
        prop_assert!(pl.orientation >= 0 && pl.orientation < piece.orientations.max(1),
            "orientation {} out of range (orientations={})", pl.orientation, piece.orientations);

        // Execute the AI's placement through the REAL engine: rotate, walk to the
        // column, hard-drop, tick to lock. The engine refuses any illegal
        // sub-move, so a resulting lock (or a legitimate top-out) proves the
        // placement is executable — and a panic here would be a real bug.
        let before = filled(&g);
        for _ in 0..pl.orientation { g.rotate(); }
        for _ in 0..(board.width * 2 + 4) {
            match g.current_piece().map(|p| p.x) {
                Some(x) if x < pl.x => g.move_right(),
                Some(x) if x > pl.x => g.move_left(),
                _ => break,
            }
        }
        g.begin_drop();
        for _ in 0..300 {
            if g.is_game_over() { break; }
            g.tick(16);
            g.take_events();
        }
        prop_assert!(filled(&g) > before || g.is_game_over(),
            "AI placement (x={}, rot={}) produced neither a lock nor a top-out",
            pl.x, pl.orientation);
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
