//! Property-based tests for bt-ai placement search and Computer driver.
//!
//! Uses proptest to verify:
//!   (a) best_placement never panics on any (board, piece).
//!   (b) The returned Placement is legal — piece fits at (x, orientation).
//!   (c) Computer::take_turn never panics and actually changes game state.

use bt_ai::{best_placement, eval_board, Computer, Placement};
use bt_core::{Board, Cell, Game, Piece, PieceKind};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// INDEPENDENT eval_board characterization.
//
// `best_placement_attains_global_minimum_score` recomputes the minimum with the
// SAME `eval_board`, so a mutant `eval_board -> 0.0` (or any monotone rescaling)
// passes it — both the candidate score and the recomputed min collapse together.
// These pin the MEANING of eval_board WITHOUT routing through best_placement, by
// constructing two hand-built boards that differ in exactly one structural way
// and asserting the score moves the RIGHT direction. A constant/zeroed eval, a
// dropped term, or a flipped sign breaks at least one of them.
// ---------------------------------------------------------------------------

/// A solid (color) cell for stacking test boards.
fn brick() -> Cell {
    Cell::color(1)
}

/// Fill column `x` of `board` solidly from row `top_y` down to the floor.
fn fill_column_from(board: &mut Board, x: i32, top_y: i32) {
    for y in top_y..board.height {
        board.set(x, y, Some(brick()));
    }
}


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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// A board with a BURIED HOLE scores strictly WORSE (higher) than the same
    /// board with the hole filled. We hold the column TOP constant across both
    /// boards (so variance + height penalties are identical), so the ONLY delta
    /// is the hole penalty. Kills `eval_board -> 0.0`, a dropped hole term, or a
    /// hole penalty whose sign rewards holes.
    #[test]
    fn buried_hole_scores_strictly_worse(
        cx in 0i32..10,
        // Stack height above the hole (>=1 so the hole is genuinely covered).
        cover in 1i32..6,
    ) {
        let h = 28;
        // hole sits just under the cover; floor cell stays filled so the top of
        // the column is identical in both boards.
        let top_y = h - cover - 2;       // topmost filled row (same in both)
        let hole_y = h - cover - 1;      // the cell we toggle
        prop_assume!(top_y >= 0 && hole_y < h - 1);

        // no_hole: column solid from top_y to floor (no empty cells below top).
        let mut no_hole = Board::standard(false);
        fill_column_from(&mut no_hole, cx, top_y);

        // with_hole: same, but punch out hole_y (covered by >=1 brick above it).
        let mut with_hole = no_hole.clone();
        with_hole.set(cx, hole_y, None);

        // Sanity: tops are identical (top filled cell unchanged).
        prop_assert!(with_hole.occupied(cx, top_y) && no_hole.occupied(cx, top_y),
            "column top must be filled in both boards");
        prop_assert!(!with_hole.occupied(cx, hole_y), "the hole must actually be empty");

        let s_no_hole = eval_board(&no_hole);
        let s_with_hole = eval_board(&with_hole);
        prop_assert!(
            s_with_hole > s_no_hole + 1.0,
            "a buried hole must raise the score: with_hole={} vs no_hole={} (col {}, cover {})",
            s_with_hole, s_no_hole, cx, cover
        );
    }

    /// A LOWER, FLATTER stack scores better (lower) than a TALLER, ROUGHER one.
    /// Two sub-claims in one construction:
    ///  * Lower is better: a 1-high single-column stack vs an H-high one (same
    ///    column) — the taller stack has the larger height + variance penalties.
    ///  * No full rows in either (only one column filled), so the line bonus
    ///    never confounds it.
    #[test]
    fn taller_stack_scores_worse(
        cx in 0i32..10,
        tall in 6i32..24,
    ) {
        let h = 28;
        // short: just the floor cell in column cx.
        let mut short = Board::standard(false);
        short.set(cx, h - 1, Some(brick()));
        // tall: column cx filled `tall` rows high (top at h-tall).
        let mut taller = Board::standard(false);
        fill_column_from(&mut taller, cx, h - tall);

        prop_assert!(
            eval_board(&taller) > eval_board(&short) + 1.0,
            "taller stack must score worse: tall={} short={} (col {}, height {})",
            eval_board(&taller), eval_board(&short), cx, tall
        );
    }

    /// A FLAT full-width-but-one stack scores better than a ROUGH one of the same
    /// max height (variance term). Both leave column 9 empty so NO row is ever
    /// complete (no line bonus) and neither has holes (each column is solid from
    /// its own top to the floor). Same global top (max height k), so the height
    /// penalty is identical — only the variance differs.
    #[test]
    fn rough_surface_scores_worse_than_flat(
        k in 4i32..18,
    ) {
        let h = 28;
        let flat_top = h - k;
        // flat: columns 0..=8 all filled to the same height; column 9 empty.
        let mut flat = Board::standard(false);
        for x in 0..9 {
            fill_column_from(&mut flat, x, flat_top);
        }
        // rough: columns 0..=8 alternate between max height k and a low stub
        // (height 1), so the surface is jagged. Same MAX height (a full-k column
        // exists), column 9 empty. No holes (each column solid to the floor).
        let mut rough = Board::standard(false);
        for x in 0..9 {
            let col_top = if x % 2 == 0 { flat_top } else { h - 1 };
            fill_column_from(&mut rough, x, col_top);
        }
        // Confirm the max height matches (so global top, hence height_pen, agrees).
        prop_assert!(rough.occupied(0, flat_top), "rough must reach the flat max height");

        prop_assert!(
            eval_board(&rough) > eval_board(&flat) + 1.0,
            "a rough surface must score worse than a flat one of equal max height: \
             rough={} flat={} (k={})",
            eval_board(&rough), eval_board(&flat), k
        );
    }

    /// A board with a COMPLETED line scores better (lower) than the same board
    /// one cell short of completing it. Construction: a SOLID block of `rows`
    /// full rows sitting on the floor (so there are no holes underneath) vs the
    /// same block with one corner cell of the TOP row removed — nothing is above
    /// it, so the incomplete board just has one fewer complete line (and no
    /// covered hole at the gap). The block is tall enough that its top is ABOVE
    /// the midline (top_row < MIDLINE = 14), where `eval_board` rewards each
    /// cleared line with a POSITIVE bonus it SUBTRACTS — so the completed board,
    /// with one extra line, must score strictly lower. (Below the midline the
    /// original deliberately PENALISES sub-tetris clears, so this property
    /// targets the rewarding branch.) Kills a dropped/sign-flipped line bonus.
    #[test]
    fn completing_a_line_scores_better(
        rows in 15i32..24,
        gap in 0i32..10,
    ) {
        let w = 10;
        let h = 28;
        let top_row = h - rows; // topmost full row of the block (< MIDLINE)
        prop_assert!(top_row < 14, "block top must be above the midline for the rewarding branch");
        // complete: a solid block of `rows` full rows resting on the floor.
        let mut complete = Board::standard(false);
        for y in top_row..h {
            for x in 0..w {
                complete.set(x, y, Some(brick()));
            }
        }
        // incomplete: remove the gap cell from the TOP row — nothing is above it,
        // so it's not a covered hole; the column's top simply drops by one and
        // that row is no longer complete.
        let mut incomplete = complete.clone();
        incomplete.set(gap, top_row, None);

        // Sanity: complete has `rows` full rows; incomplete has one fewer.
        let full_rows = |b: &Board| (0..h).filter(|&y| (0..w).all(|x| b.occupied(x, y))).count();
        prop_assert_eq!(full_rows(&complete), rows as usize, "complete must have all rows full");
        prop_assert_eq!(full_rows(&incomplete), (rows - 1) as usize,
            "incomplete must have exactly one fewer full row");

        prop_assert!(
            eval_board(&complete) < eval_board(&incomplete),
            "a completed line must score better than an incomplete one: \
             complete={} incomplete={} (rows {}, gap {})",
            eval_board(&complete), eval_board(&incomplete), rows, gap
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `take_turn` must actually STEER the piece to `best_placement`'s target,
    /// not just hard-drop it where it spawned. "Anything changed" (above) is too
    /// weak: a driver that calls `ai_begin_drop()` without any rotate/move passes
    /// it (the falling piece still moves down). Here we compute the target
    /// independently, and on a board where the target (x, orientation) DIFFERS
    /// from the spawn pose, we assert the piece reached EXACTLY that pose BEFORE
    /// the drop tick (begin_drop only engages the fast cadence — it doesn't move
    /// the piece until the next tick). A "skip the alignment" mutant leaves the
    /// piece at spawn and fails.
    #[test]
    fn take_turn_steers_piece_to_best_placement(
        seed in any::<u64>(),
        pre_ticks in 0u64..500u64,
    ) {
        let mut g = Game::new(seed);
        for _ in 0..pre_ticks {
            if g.is_game_over() { break; }
            g.tick(16);
        }
        if g.is_game_over() { return Ok(()); }
        let Some(piece) = g.current_piece().cloned() else { return Ok(()); };

        // Independent target (same search the driver uses).
        let target = best_placement(g.board(), &piece);
        let (spawn_x, spawn_o) = (piece.x, piece.orientation);

        // Only interesting when the AI must MOVE/ROTATE off the spawn pose.
        prop_assume!((target.x, target.orientation) != (spawn_x, spawn_o));

        let mut ernie = Computer::new();
        ernie.take_turn(&mut g);

        // Right after take_turn (no tick yet) the piece must sit at the target —
        // the driver rotated then slid it there. (If the slide were blocked the
        // target wouldn't have been an enterable/reachable best_placement; the
        // companion enterability property guards that.)
        let Some(after) = g.current_piece() else { return Ok(()); };
        prop_assert_eq!(
            (after.x, after.orientation),
            (target.x, target.orientation),
            "take_turn did not steer to best_placement: spawn=({},{}) target=({},{}) got=({},{})",
            spawn_x, spawn_o, target.x, target.orientation, after.x, after.orientation
        );
        // And it genuinely left the spawn pose (non-vacuity).
        prop_assert_ne!((after.x, after.orientation), (spawn_x, spawn_o),
            "take_turn left the piece at its spawn pose despite a differing target");
    }
}

// ---------------------------------------------------------------------------
// eval_board GOLDEN FIXTURES (exact numeric values).
//
// The directional properties above pin the SHAPE of the heuristic, and the
// `eval_penalty_weights_match_btcomputer` unit test pins the named CONSTANTS, but
// neither pins that the FORMULA assembles them correctly (right coefficients /
// powers / branch). A mutant like `height_pen = fraction^3 * HEIGHT_PENALTY`
// (wrong power) or `variance_raw * 50.0 * (1-f)^2` (wrong cubic) keeps the
// constants and the directional orderings, yet changes the number. These hand-
// derived exact values (from the BTCBoard::eval formula) catch that.
#[test]
fn eval_board_matches_hand_derived_golden_values() {
    let approx = |got: f64, want: f64, label: &str| {
        assert!((got - want).abs() < 1e-6, "{label}: eval_board = {got}, expected {want}");
    };

    // (1) Empty board: no variance, no holes, top == h so height fraction = 0,
    //     no lines. eval == 0 exactly.
    let empty = Board::standard(false);
    approx(eval_board(&empty), 0.0, "empty board");

    // (2) A single filled FLOOR cell at (0, 27):
    //     top=27, fraction_top=27/28.
    //     variance_raw = 2 (the C++ ptops_[1]-seeded scan over one step at each
    //       edge of the lone column), variance = 2*50*(1/28)^3.
    //     height_pen = (1/28)^2 * 30000.  No holes, no lines.
    let mut one = Board::standard(false);
    one.set(0, 27, Some(brick()));
    let f = 1.0_f64 / 28.0;
    let want_one = 2.0 * 50.0 * f * f * f + f * f * 30_000.0;
    approx(eval_board(&one), want_one, "single floor cell");

    // (3) A complete bottom row (all 10 cols at y=27):
    //     top=27, variance 0 (flat), height_pen = (1/28)^2*30000, ONE full line.
    //     top(27) is NOT < MIDLINE(14), so the line bonus uses the ELSE branch:
    //       LINE_BONUS * (-4 + 1) * fraction_top = 5000 * -3 * 27/28, and eval
    //       SUBTRACTS it (so it adds +14464.2857…).
    let mut row = Board::standard(false);
    for x in 0..10 { row.set(x, 27, Some(brick())); }
    let ftop = 27.0_f64 / 28.0;
    let height_pen = f * f * 30_000.0;
    let line_bonus = 5_000.0 * (-4.0 + 1.0) * ftop; // negative
    let want_row = height_pen - line_bonus;
    approx(eval_board(&row), want_row, "full bottom row");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// `take_turn` must engage Ernie's FLAT placement score (`ai_begin_drop` ->
    /// `BT_BOARD_HGT / 2`, BTComputer.C:1255), NOT the human hard-drop bonus
    /// (`begin_drop` -> `BT_BOARD_HGT - y`). On a FRESH game the piece is at the
    /// spawn row (y = 0), so the human bonus would be `BT_BOARD_HGT` (= 28) while
    /// the AI's flat award is `BT_BOARD_HGT / 2` (= 14) — distinct values. We
    /// assert the score jumps by EXACTLY the flat amount when take_turn fires the
    /// drop, killing an `ai_begin_drop()` -> `begin_drop()` substitution.
    #[test]
    fn take_turn_uses_the_flat_ai_drop_score(seed in any::<u64>()) {
        use bt_core::constants::BT_BOARD_HGT;
        let mut g = Game::new(seed);
        // Fresh game: the piece is at spawn (y = 0). take_turn rotates/slides (no
        // vertical move) then fires the drop, so the score award is computed at y=0.
        prop_assert_eq!(g.piece_pos().1, 0, "fresh piece must be at the spawn row");
        let score_before = g.score().score;

        let mut ernie = Computer::new();
        ernie.take_turn(&mut g); // engages ai_begin_drop (synchronous score award)

        let delta = g.score().score - score_before;
        prop_assert_eq!(delta, (BT_BOARD_HGT / 2) as i64,
            "take_turn must award the FLAT AI drop score {} (not the human {}-y bonus); got {}",
            BT_BOARD_HGT / 2, BT_BOARD_HGT, delta);
    }
}
