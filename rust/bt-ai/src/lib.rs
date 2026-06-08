//! `bt-ai` — the BattleTris computer opponent, "Ernie".
//!
//! A faithful Rust port of `BTComputer` + `BTCBoard` from
//! `usr/src/game/BTComputer.{H,C}` and `BTCBoard.{H,C}`.
//!
//! ## Design summary
//!
//! * [`eval_board`] — board heuristic. Ports `BTCBoard::eval` with its five
//!   dominant terms: variance (column-height roughness), covered-hole penalty,
//!   open/closed hole penalty, height penalty, and line bonus. Lower = better.
//!
//! * [`best_placement`] — exhaustive orientation × column search. For each
//!   candidate `(orientation, x)` it clones the board, rotates, slides, drops
//!   the piece, calls `eval_board`, and keeps the minimum. Equivalent to the
//!   `decide()` / `checkMove()` / `computeValue()` loop in BTComputer.C.
//!
//! * [`Computer`] — drives a [`bt_core::Game`] turn-by-turn: `take_turn`
//!   queries the current piece, finds the best placement, then steers the game
//!   with `rotate()` / `move_left()` / `move_right()` / `ai_begin_drop()`.
//!
//! ## Eval formula (from BTCBoard.C / BTComputer.C)
//!
//! ```text
//! value = variance + cov_hole_pen + hole_pen + height_pen - line_bonus
//! ```
//!
//! Constants (from BTComputer.C `#define` block):
//! * `OPEN_HOLE_PENALTY`    = 7 000
//! * `CLOSED_HOLE_PENALTY`  = 10 000
//! * `COVERED_HOLE_PENALTY` = 3 000
//! * `HEIGHT_PENALTY`       = 30 000
//! * `LINE_BONUS`           = 5 000
//! * `HAPPY_BONUS`          = 20 000   (unused — we skip happy-piece logic)
//! * `VARIANCE_PENALTY`     = 50
//! * `HOLE_DECAY`           = 0.50

use bt_core::{Board, Game, Piece};

mod vs;
pub use vs::{VsComputer, AI_LAUNCH_PERIOD_MS, AI_LEVELS};

/// Smarter weapon buying/launching policy for the networked bots (`bt-bot`).
pub mod weapons;

// ---------------------------------------------------------------------------
// Penalty constants — from BTComputer.C
// ---------------------------------------------------------------------------

/// Penalty per open hole (empty square that can be reached from the side
/// without passing through a filled square).
const OPEN_HOLE_PENALTY: f64 = 7_000.0;

/// Penalty per closed hole (empty square enclosed on all reachable sides).
const CLOSED_HOLE_PENALTY: f64 = 10_000.0;

/// Multiplier for covered-hole penalty accumulated per block above a hole.
/// Weighted by `HOLE_DECAY^depth * blocks_above`.
const COVERED_HOLE_PENALTY: f64 = 3_000.0;

/// Height penalty scaling factor. Applied as `fraction^2 * HEIGHT_PENALTY`
/// where `fraction = 1 - (landing_row / board_height)`.
const HEIGHT_PENALTY: f64 = 30_000.0;

/// Bonus per line cleared. Applied as `LINE_BONUS * lines * (1 - fraction)`.
const LINE_BONUS: f64 = 5_000.0;

/// Variance-penalty scaling factor (multiplied by additional cubic fraction
/// terms exactly as in BTCBoard::eval).
const VARIANCE_PENALTY: f64 = 50.0;

/// Exponential decay per row of depth for the covered-hole term.
const HOLE_DECAY: f64 = 0.50;

/// Spawn-row midline (BT_MIDLINE in BTComputer.C) — if the board top is
/// above this row the line bonus formula switches branches.
const MIDLINE: i32 = 14;

// ---------------------------------------------------------------------------
// eval_board
// ---------------------------------------------------------------------------

/// Board heuristic: **lower is better**.
///
/// Faithful port of `BTCBoard::eval` (BTCBoard.C). Works in two passes:
///
/// **Pass 1 — column heights and line detection.**
/// Scans every cell, records per-column top rows, simulates line clears
/// (without actually mutating the board), and counts lines that would be
/// cleared.
///
/// **Pass 2 — hole detection.**
/// An empty cell is a *hole* if there is at least one filled cell directly
/// above it in the same column. A hole is *covered* if the piece (not modelled
/// here — we work post-landing) overlaps its column. We approximate the
/// covered-hole term by accumulating decay × filled-cells-above for each hole.
/// Open vs. closed classification: a hole is *open* if it has an unobstructed
/// horizontal escape to the left or right column edge without passing through
/// filled cells; otherwise it is *closed* (more expensive).
///
/// **Terms assembled:**
/// ```text
/// value = variance + cov_hole_pen + hole_pen + height_pen - line_bonus
/// ```
pub fn eval_board(board: &Board) -> f64 {
    let w = board.width;
    let h = board.height;

    // --- Pass 1: per-column top rows (smallest y = highest cell) ----------
    // tops_[x] = y of the topmost occupied cell in column x, or h if empty.
    let mut tops = vec![h; w as usize];
    for x in 0..w {
        for y in 0..h {
            if board.occupied(x, y) {
                tops[x as usize] = y;
                break;
            }
        }
    }

    // Global top (minimum of per-column tops).
    let top = *tops.iter().min().unwrap_or(&h);

    // --- Variance (BTCBoard::variance) ------------------------------------
    // The original computes sum of squared differences between adjacent
    // column tops (including repeating the last column once):
    //
    //   prev_height = ptops_[1]   (note: *index 1*, not 0 — faithful to C++)
    //   for j in 0..BT_BOARD_WTH:
    //       temp2 = abs(ptops_[j] - prev_height)^2
    //       temp  += temp2
    //       prev_height = ptops_[j]
    //   temp += temp2   // repeats last column
    //
    // ptops_ after a piece-land mirrors tops_ (no active piece in our case).
    let variance_raw = {
        let mut temp: f64 = 0.0;
        let mut last_sq: f64 = 0.0;
        // C++ seeds prev_height = ptops_[1] (column index 1)
        let mut prev = tops[1] as f64;
        for &col_top in tops.iter().take(w as usize) {
            let h = col_top as f64;
            let diff = (h - prev).abs();
            last_sq = diff * diff;
            temp += last_sq;
            prev = h;
        }
        temp += last_sq; // "account for furthest column"
        temp
    };

    // Fraction term for variance: (1 - top/h)^3
    // BTCBoard::eval: variance_ = variance() * vp * (1-f)^3  where f = top/h
    let fraction_top = top as f64 / h as f64;
    let variance = variance_raw * VARIANCE_PENALTY
        * (1.0 - fraction_top)
        * (1.0 - fraction_top)
        * (1.0 - fraction_top);

    // --- Pass 2: holes + height -------------------------------------------
    // A hole is an empty cell that has at least one filled cell above it
    // in the same column (i.e., y > tops_[x]).
    //
    // Open hole: the cell has horizontal access to the board edge without
    // crossing a filled cell (simplified: any empty cell in the same row
    // to left or right before hitting a filled cell or the wall).
    //
    // Covered-hole term: for each hole, accumulate decay^depth * blocks_above,
    // where depth = hole_y - tops_[x] - 1 and blocks_above = number of filled
    // cells above the hole in the same column (= hole_y - tops_[x]).
    //
    // This mirrors cboard_.eval()'s covered_holes_ / cov_hole_pen_ accounting.

    let mut open_holes: i32 = 0;
    let mut closed_holes: i32 = 0;
    let mut cov_hole_pen: f64 = 0.0;

    for x in 0..w {
        let col_top = tops[x as usize];
        if col_top >= h {
            continue; // empty column, no holes
        }
        for y in (col_top + 1)..h {
            if board.occupied(x, y) {
                continue; // filled cell, not a hole
            }
            // Empty cell below the column top → it's a hole.
            let depth = (y - col_top - 1) as f64; // 0-based depth below first filled
            let blocks_above = (y - col_top) as f64; // filled cells strictly above

            // Covered-hole contribution (decay by depth × blocks above)
            let decay = HOLE_DECAY.powf(depth);
            cov_hole_pen += decay * blocks_above;

            // Open vs. closed: scan left and right in this row for an escape.
            let is_open = {
                let mut open = false;
                // Scan left
                let mut lx = x - 1;
                while lx >= 0 {
                    if board.occupied(lx, y) {
                        break;
                    }
                    if lx == 0 {
                        open = true; // reached left wall via empty cells
                        break;
                    }
                    // if tops[lx] >= y there's no block above here — it's a
                    // gap that connects to open space
                    if tops[lx as usize] > y {
                        open = true;
                        break;
                    }
                    lx -= 1;
                }
                if !open {
                    // Scan right
                    let mut rx = x + 1;
                    while rx < w {
                        if board.occupied(rx, y) {
                            break;
                        }
                        if rx == w - 1 {
                            open = true;
                            break;
                        }
                        if tops[rx as usize] > y {
                            open = true;
                            break;
                        }
                        rx += 1;
                    }
                }
                open
            };

            if is_open {
                open_holes += 1;
            } else {
                closed_holes += 1;
            }
        }
    }

    let hole_pen = closed_holes as f64 * CLOSED_HOLE_PENALTY
        + open_holes as f64 * OPEN_HOLE_PENALTY;
    cov_hole_pen *= COVERED_HOLE_PENALTY;

    // --- Height penalty ---------------------------------------------------
    // BTCBoard::eval: fraction = 1 - (j + piece_top) / h
    //                 height_pen = fraction^2 * hp
    // We use the global top (no active piece context).
    let fraction_height = 1.0 - (top as f64 / h as f64);
    let height_pen = fraction_height * fraction_height * HEIGHT_PENALTY;

    // --- Line bonus -------------------------------------------------------
    // Count lines that would be cleared if the board were checked right now.
    // BTCBoard::eval: lines_cleared_ counts full rows; then
    //   if no_tetri || top < midline:  lb * lines * (1 - fraction)
    //   else:                          lb * (-4 + lines) * fraction
    // We conservatively use the first branch (no_tetri=true equiv).
    let lines_cleared = {
        let mut n = 0i32;
        for y in 0..h {
            let full = (0..w).all(|x| board.occupied(x, y));
            if full {
                n += 1;
            }
        }
        n
    };

    let line_bonus = if lines_cleared > 0 {
        let no_tetri = top < MIDLINE; // conservative: always use safe branch
        if no_tetri || top < MIDLINE {
            LINE_BONUS * lines_cleared as f64 * (1.0 - fraction_top)
        } else {
            LINE_BONUS * (-4.0 + lines_cleared as f64) * fraction_top
        }
    } else {
        0.0
    };

    variance + cov_hole_pen + hole_pen + height_pen - line_bonus
}

/// Strong, line-clearing board evaluation for the ONLINE BOTS — NOT the faithful
/// single-player Ernie (which deliberately hoards Tetrises and so clears poorly).
/// The classic Lee 4-feature heuristic with its well-known tuned weights:
/// aggregate column height, rows cleared by this placement, covered holes, and
/// surface bumpiness. **Higher is better** (opposite sign convention to
/// `eval_board`). 1-ply, but it stacks flat and clears lines steadily.
///
/// `lines_cleared` is how many rows this placement completed — it MUST be counted
/// before `check_lines()` removes them (the board passed here is already cleared).
pub fn eval_board_strong(board: &Board, lines_cleared: i32) -> f64 {
    let w = board.width;
    let h = board.height;
    let mut heights = vec![0i32; w as usize];
    let mut holes = 0i32;
    for x in 0..w {
        // First filled row from the top; `h` means the column is empty.
        let mut top = h;
        for y in 0..h {
            if board.occupied(x, y) {
                top = y;
                break;
            }
        }
        heights[x as usize] = h - top;
        if top < h {
            for y in (top + 1)..h {
                if !board.occupied(x, y) {
                    holes += 1;
                }
            }
        }
    }
    let agg: i32 = heights.iter().sum();
    let mut bump = 0i32;
    for x in 0..(w as usize - 1) {
        bump += (heights[x] - heights[x + 1]).abs();
    }
    // Lee's genetic-algorithm weights (en.wikipedia.org/wiki/Tetris + the well-known
    // "el-tetris"/Lee writeups). Survives indefinitely and clears lines greedily.
    -0.510066 * agg as f64 + 0.760666 * lines_cleared as f64
        - 0.356630 * holes as f64
        - 0.184483 * bump as f64
}

// ---------------------------------------------------------------------------
// Placement search
// ---------------------------------------------------------------------------

/// The best column × orientation to place `piece` on `board`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// Leftmost x of the piece's local grid when it locks.
    pub x: i32,
    /// Number of clockwise rotations from the spawn orientation.
    pub orientation: i32,
}

/// Find the best placement for `piece` on `board`.
///
/// Exhaustively tries every orientation in `0..piece.orientations` and every
/// column position. For each candidate:
///
/// 1. Clone the board and piece.
/// 2. Rotate the piece clone `orientation` times.
/// 3. Move the piece to the candidate x at the spawn row (`y = 0`).
/// 4. Drop it down (`move_to` incrementing y until it fails).
/// 5. Land it into the cloned board.
/// 6. `eval_board` the result.
///
/// Returns the `{x, orientation}` with the lowest (best) eval score.
/// Falls back to `{x: piece.x, orientation: 0}` (the spawn position) if no
/// candidate placement succeeds.
///
/// This is the spirit of `BTComputer::decide()` + `computeValue()` in
/// BTComputer.C: a full column × orientation simulation.
///
/// ## Tie-breaking
/// The C++ `checkMove` DFS starts at `def_x_` (centre) and expands outward, so
/// ties resolve in favour of the centre column. We reproduce that centre bias by
/// visiting candidates centre-out — 5, 6, 4, 7, 3, 8, 2, 9, 1, 0, extended with
/// negative offsets — and keeping the FIRST best on ties (strict `<`). What we
/// pin is the centre-out priority, not the C++ DFS's exact intra-cell direction
/// order; the centre column wins ties either way.
pub fn best_placement(board: &Board, piece: &Piece) -> Placement {
    let mut best_score = f64::MAX;
    let mut best = Placement { x: piece.x, orientation: 0 };
    // Strict less-than preserves first-found (centre) on ties — faithful to
    // `checkMove`'s centre-out DFS. eval_board: LOWER is better.
    for_each_landing(board, piece, |pl, b, _lines| {
        let score = eval_board(b);
        if score < best_score {
            best_score = score;
            best = pl;
        }
    });
    best
}

/// Like [`best_placement`] but uses the strong, line-clearing [`eval_board_strong`]
/// (HIGHER is better) — for the ONLINE BOTS, leaving faithful Ernie untouched.
pub fn best_placement_strong(board: &Board, piece: &Piece) -> Placement {
    let mut best_score = f64::MIN;
    let mut best = Placement { x: piece.x, orientation: 0 };
    for_each_landing(board, piece, |pl, b, lines| {
        let score = eval_board_strong(b, lines);
        if score > best_score {
            best_score = score;
            best = pl;
        }
    });
    best
}

/// How much noise a skill of 0 adds to each candidate's strong-eval score. Tuned so
/// skill≈0 plays roughly like (or a touch below) faithful Ernie and skill 1 is the
/// full strong eval — a smooth difficulty dial for the rating-matched roaming bot.
const SKILL_NOISE_SCALE: f64 = 8.0;

/// A tiny xorshift step → a float in `[0,1)`. Used ONLY for the skill-noise above, so
/// it stays out of the engine's deterministic piece RNG (mixing them would desync
/// the bot's predicted board from the server's). Seed must be non-zero.
fn xorshift01(state: &mut u64) -> f64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}

/// Skill-scaled placement for a rating-matched bot. `skill` ∈ `[0,1]`: 1.0 returns the
/// strong best; lower skill adds proportional noise to each candidate's score so the
/// bot increasingly settles for a plausible-but-worse landing (a weaker opponent).
/// `rng` is a caller-owned xorshift seed, NOT the engine RNG.
pub fn best_placement_skill(board: &Board, piece: &Piece, skill: f64, rng: &mut u64) -> Placement {
    let skill = skill.clamp(0.0, 1.0);
    if skill >= 0.999 {
        return best_placement_strong(board, piece);
    }
    let noise_amp = (1.0 - skill) * SKILL_NOISE_SCALE;
    let mut best_score = f64::MIN;
    let mut best = Placement { x: piece.x, orientation: 0 };
    for_each_landing(board, piece, |pl, b, lines| {
        let n = (xorshift01(rng) - 0.5) * 2.0 * noise_amp;
        let score = eval_board_strong(b, lines) + n;
        if score > best_score {
            best_score = score;
            best = pl;
        }
    });
    best
}

/// Enumerate every valid hard-drop landing of `piece` on `board` in the faithful
/// centre-out order (`def_x_=5`, right-before-left), calling
/// `visit(placement, &landed_board, lines_cleared)` for each. `landed_board` has the
/// piece locked and full rows already cleared; `lines_cleared` is how many rows that
/// placement completed, counted BEFORE the clear (the faithful eval re-counts
/// post-clear → 0, so only the strong eval consumes it). Shared by both
/// `best_placement` and `best_placement_strong`.
fn for_each_landing(board: &Board, piece: &Piece, mut visit: impl FnMut(Placement, &Board, i32)) {
    let spawn_y = 0i32;

    // Candidate columns, centre-out from `def_x_=5` — the centre bias that
    // `checkMove`'s outward DFS gives ties.
    let centre = board.width / 2; // = 5 for the standard board
    let search_min = -(bt_core::constants::BT_PIECE_WIDTH as i32 - 1);
    let search_max = board.width; // exclusive
    let mut candidates: Vec<i32> = Vec::with_capacity((search_max - search_min) as usize);
    candidates.push(centre);
    let mut d = 1i32;
    loop {
        let added = [centre - d, centre + d]
            .iter()
            .copied()
            .filter(|&x| x >= search_min && x < search_max)
            .collect::<Vec<_>>();
        if added.is_empty() {
            break;
        }
        // Right before left at each distance — a fixed, deterministic tie-break
        // order (the centre column is still visited first, so centre wins ties).
        for x in added.iter().rev() {
            if *x >= search_min && *x < search_max {
                candidates.push(*x);
            }
        }
        d += 1;
        if centre - d < search_min && centre + d >= search_max {
            break;
        }
    }

    for o in 0..piece.orientations {
        // Rotate a clone `o` times to get the rotated shape (safe scratch board).
        let rotated_piece = {
            let scratch = Board::new(board.width + 20, board.height + 20, true);
            let mut p = piece.clone();
            p.move_to(&scratch, 10, 10);
            for _ in 0..o {
                if !p.rotate(&scratch, false) {
                    break;
                }
            }
            p
        };

        for &candidate_x in &candidates {
            let mut b = board.clone();
            let mut p = rotated_piece.clone();

            if !p.move_to(&b, candidate_x, spawn_y) {
                continue;
            }
            // Hard drop: descend until blocked.
            let mut landed_y = spawn_y;
            while p.can_move_to(&b, candidate_x, landed_y + 1) {
                landed_y += 1;
            }
            if !p.move_to(&b, candidate_x, landed_y) {
                continue;
            }
            p.land(&mut b);

            // Count completed rows BEFORE clearing them.
            let mut lines = 0i32;
            for y in 0..b.height {
                if (0..b.width).all(|x| b.occupied(x, y)) {
                    lines += 1;
                }
            }
            b.check_lines();

            visit(Placement { x: candidate_x, orientation: o }, &b, lines);
        }
    }
}

// ---------------------------------------------------------------------------
// Computer driver
// ---------------------------------------------------------------------------

/// The Ernie computer player.
///
/// Mirrors `BTComputer` in BTComputer.C. `take_turn` drives a single piece to
/// the best placement computed by [`best_placement`].
#[derive(Clone, Debug)]
pub struct Computer {
    // Reserved for future difficulty / move-delay extension.
    _priv: (),
}

impl Computer {
    /// Create a new Computer player (equivalent to `BTComputer::reset()`).
    pub fn new() -> Computer {
        Computer { _priv: () }
    }

    /// Drive `game`'s current piece to the best placement.
    ///
    /// Reads `game.current_piece()`, computes the best `{x, orientation}` via
    /// [`best_placement`], then:
    ///
    /// 1. Calls `game.rotate()` until the piece's `orientation` matches
    ///    `best.orientation` (capped at `orientations` iterations).
    /// 2. Calls `game.move_left()` / `game.move_right()` until the piece's `x`
    ///    matches `best.x` (capped to avoid infinite loops).
    /// 3. Calls `game.ai_begin_drop()` to hard-drop — the computer's flat
    ///    `BT_BOARD_HGT / 2` placement score (BTComputer.C:1255), NOT the
    ///    human's variable hard-drop bonus.
    ///
    /// No-ops if `game.is_game_over()` or there is no current piece.
    pub fn take_turn(&mut self, game: &mut Game) {
        if game.is_game_over() {
            return;
        }
        let piece = match game.current_piece() {
            Some(p) => p.clone(),
            None => return,
        };

        let placement = best_placement(game.board(), &piece);

        // --- Rotate to target orientation ---
        let orientations = piece.orientations.max(1);
        for _ in 0..orientations {
            let cur_orient = match game.current_piece() {
                Some(p) => p.orientation,
                None => return,
            };
            if cur_orient == placement.orientation {
                break;
            }
            game.rotate();
        }

        // --- Slide to target x ---
        // Cap moves at board_width * 2 to prevent infinite loops.
        let max_moves = game.board().width * 2;
        for _ in 0..max_moves {
            let cur_x = match game.current_piece() {
                Some(p) => p.x,
                None => return,
            };
            if cur_x == placement.x {
                break;
            }
            if cur_x < placement.x {
                game.move_right();
            } else {
                game.move_left();
            }
        }

        // --- Hard drop ---
        // Use the computer's placement scoring (flat BT_BOARD_HGT/2), not the
        // human hard-drop bonus the human `begin_drop` would award.
        game.ai_begin_drop();
    }
}

impl Default for Computer {
    fn default() -> Self {
        Computer::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bt_core::{Board, Game, Piece, PieceKind};

    // --- eval_board tests ---------------------------------------------------

    /// A board with a deep hole should score worse (higher) than one without.
    #[test]
    fn eval_hole_worse_than_clean() {
        // Build two boards: one clean column, one with a buried hole.
        let mut clean = Board::standard(true);
        let mut holey = Board::standard(true);

        // Fill the bottom 4 rows of column 0 on both boards.
        // On `holey`, skip row 23 (leaving a hole under 4 blocks).
        let w = clean.width;
        for y in 24..28i32 {
            for x in 0..w {
                clean.set(x, y, Some(bt_core::Cell::color(2)));
                holey.set(x, y, Some(bt_core::Cell::color(2)));
            }
        }
        // Bury a hole in column 5 of holey: fill rows 20-22 but not 23.
        holey.set(5, 20, Some(bt_core::Cell::color(2)));
        holey.set(5, 21, Some(bt_core::Cell::color(2)));
        holey.set(5, 22, Some(bt_core::Cell::color(2)));
        // Row 23 col 5 stays empty — that's the hole.
        holey.set(5, 24, Some(bt_core::Cell::color(2))); // already set above

        // Clean board should score better (lower).
        let s_clean = eval_board(&clean);
        let s_holey = eval_board(&holey);
        assert!(
            s_holey > s_clean,
            "holey board ({s_holey}) should score worse than clean ({s_clean})"
        );
    }

    // --- strong eval: line-clearing for the online bots --------------------

    /// Drive a solo Game with the given placement strategy for up to `max_pieces`
    /// (or until top-out), returning (total lines cleared, pieces placed).
    fn play_solo(strong: bool, seed: u64, max_pieces: usize) -> (i64, usize) {
        use bt_core::game::GameEvent;
        let mut g = Game::new(seed);
        let mut placed = 0usize;
        let mut committed = false;
        let mut guard = 0usize;
        let limit = max_pieces * 400;
        while placed < max_pieces && !g.is_game_over() && guard < limit {
            guard += 1;
            if g.is_in_bazaar() {
                g.leave_bazaar();
            }
            if !committed {
                if let Some(p) = g.current_piece().cloned() {
                    let pl = if strong {
                        best_placement_strong(g.board(), &p)
                    } else {
                        best_placement(g.board(), &p)
                    };
                    let orientations = p.orientations.max(1);
                    for _ in 0..orientations {
                        match g.current_piece() {
                            Some(cp) if cp.orientation != pl.orientation => g.rotate(),
                            _ => break,
                        }
                    }
                    let maxm = g.board().width * 2;
                    for _ in 0..maxm {
                        match g.current_piece().map(|cp| cp.x) {
                            Some(x) if x < pl.x => g.move_right(),
                            Some(x) if x > pl.x => g.move_left(),
                            _ => break,
                        }
                    }
                    g.ai_begin_drop();
                    committed = true;
                }
            }
            g.tick(16);
            if g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
                committed = false;
                placed += 1;
            }
        }
        (g.score().lines, placed)
    }

    #[test]
    fn strong_eval_outclears_faithful_ernie() {
        let mut strong_total = 0i64;
        let mut ernie_total = 0i64;
        for seed in [1u64, 7, 42, 100, 2024] {
            let (s, sp) = play_solo(true, seed, 250);
            let (e, ep) = play_solo(false, seed, 250);
            println!("seed {seed}: strong {s} lines / {sp} placed    ernie {e} lines / {ep} placed");
            // The strong (Lee) eval must never top itself out in solo and must clear
            // lines steadily — this is the robustness faithful Ernie lacks (it tops
            // out on several seeds, which reads as "doesn't clear lines well").
            assert_eq!(sp, 250, "strong eval topped out on seed {seed} ({sp} placed)");
            assert!(s >= 50, "strong eval cleared too few lines on seed {seed}: {s}");
            strong_total += s;
            ernie_total += e;
        }
        println!("TOTAL: strong {strong_total} lines    ernie {ernie_total} lines");
        assert!(
            strong_total > ernie_total,
            "strong eval should clear more lines (strong {strong_total} vs ernie {ernie_total})"
        );
    }

    /// Drive a solo Game at a given skill (the rating-matched dial), returning lines.
    fn play_solo_skill(skill: f64, seed: u64, max_pieces: usize) -> i64 {
        use bt_core::game::GameEvent;
        let mut g = Game::new(seed);
        let mut rng: u64 = (seed ^ 0x9E37_79B9_7F4A_7C15) | 1; // non-zero xorshift seed
        let mut placed = 0usize;
        let mut committed = false;
        let mut guard = 0usize;
        let limit = max_pieces * 400;
        while placed < max_pieces && !g.is_game_over() && guard < limit {
            guard += 1;
            if g.is_in_bazaar() {
                g.leave_bazaar();
            }
            if !committed {
                if let Some(p) = g.current_piece().cloned() {
                    let pl = best_placement_skill(g.board(), &p, skill, &mut rng);
                    for _ in 0..p.orientations.max(1) {
                        match g.current_piece() {
                            Some(cp) if cp.orientation != pl.orientation => g.rotate(),
                            _ => break,
                        }
                    }
                    for _ in 0..g.board().width * 2 {
                        match g.current_piece().map(|cp| cp.x) {
                            Some(x) if x < pl.x => g.move_right(),
                            Some(x) if x > pl.x => g.move_left(),
                            _ => break,
                        }
                    }
                    g.ai_begin_drop();
                    committed = true;
                }
            }
            g.tick(16);
            if g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
                committed = false;
                placed += 1;
            }
        }
        g.score().lines
    }

    #[test]
    fn skill_dial_scales_line_clearing_monotonically() {
        let seeds = [1u64, 7, 42, 100, 2024];
        let mut totals = [0i64; 3]; // skill 1.0, 0.5, 0.1
        for &seed in &seeds {
            totals[0] += play_solo_skill(1.0, seed, 200);
            totals[1] += play_solo_skill(0.5, seed, 200);
            totals[2] += play_solo_skill(0.1, seed, 200);
        }
        println!("skill 1.0: {}  0.5: {}  0.1: {}", totals[0], totals[1], totals[2]);
        assert!(totals[0] > totals[1], "skill 1.0 ({}) > 0.5 ({})", totals[0], totals[1]);
        assert!(totals[1] > totals[2], "skill 0.5 ({}) > 0.1 ({})", totals[1], totals[2]);
    }

    // --- best_placement tests -----------------------------------------------

    /// On an empty board, best_placement returns a valid in-bounds x
    /// and an orientation in 0..orientations.
    #[test]
    fn best_placement_long_in_bounds() {
        let board = Board::standard(true);
        let piece = Piece::construct(PieceKind::Long, 5, 0, 1);
        let pl = best_placement(&board, &piece);
        assert!(
            pl.orientation >= 0 && pl.orientation < piece.orientations,
            "orientation {} out of range 0..{}",
            pl.orientation,
            piece.orientations
        );
        // x is the piece-local origin; it may be slightly negative (piece cells
        // can still land on the board). The important thing is at least one cell
        // lands on the board.
        let bw = board.width;
        assert!(
            pl.x >= -(bt_core::constants::BT_PIECE_WIDTH as i32 - 1) && pl.x < bw,
            "x {} wildly out of range for board width {bw}",
            pl.x
        );
    }

    #[test]
    fn best_placement_box_in_bounds() {
        let board = Board::standard(true);
        let piece = Piece::construct(PieceKind::Box, 5, 0, 1);
        let pl = best_placement(&board, &piece);
        assert!(pl.orientation >= 0 && pl.orientation < piece.orientations);
        assert!(
            pl.x >= -(bt_core::constants::BT_PIECE_WIDTH as i32 - 1)
                && pl.x < board.width
        );
    }

    // --- Computer / Game integration tests ----------------------------------

    /// Helper: run a game loop, calling `on_piece` once per new piece.
    /// Ticks until either game over or `max_pieces` pieces have been placed.
    /// Returns (locked_count, total_lines).
    fn run_game<F>(mut game: Game, mut on_piece: F) -> (usize, i32)
    where
        F: FnMut(&mut Game),
    {
        const MAX_PIECES: usize = 300;
        // Each outer loop iteration = one piece from spawn → lock.
        // We tick at 5ms per step; after begin_drop the fast speed is 10ms/row.
        // Max rows = 28, so 28 * 10ms = 280ms = 56 ticks to reach the bottom.
        // We allow 100 ticks (500ms) to be safe. After locking, the next piece
        // spawns with slow speed (512ms/row). We call on_piece immediately after
        // the spawn.
        let mut locked = 0usize;
        let mut lines = 0i32;

        for _ in 0..MAX_PIECES {
            if game.is_game_over() {
                break;
            }
            if game.current_piece().is_none() {
                break;
            }

            // Let the caller steer this piece.
            on_piece(&mut game);

            // Tick until this piece locks (fast drop is 10ms; 100 × 5ms = 500ms).
            // We watch for a Locked event to stop early.
            let mut locked_this = false;
            'fall: for _ in 0..200 {
                if game.is_game_over() {
                    break 'fall;
                }
                game.tick(5);
                for ev in game.take_events() {
                    if let bt_core::GameEvent::Locked { lines: l, .. } = ev {
                        locked += 1;
                        lines += l;
                        locked_this = true;
                        break 'fall;
                    }
                }
            }
            // If we didn't see a Locked event, drain any stale events.
            if !locked_this {
                game.take_events();
            }
        }
        (locked, lines)
    }

    /// The AI-driven game should survive at least as long as a passive
    /// (no-input) game over the same seed, measured in pieces locked.
    ///
    /// We compare:
    ///   - **AI game**: Computer::take_turn() each piece, then tick to lock.
    ///   - **Passive game**: pieces just fall with no input (tick only).
    ///
    /// The AI must last at least as many locked pieces as the passive run.
    #[test]
    fn ai_outlasts_passive() {
        const SEED: u64 = 42;

        let (passive_pieces, _) = run_game(Game::new(SEED), |_game| {
            // No input — pieces fall freely.
        });

        let mut ernie = Computer::new();
        let (ai_pieces, _) = run_game(Game::new(SEED), |game| {
            ernie.take_turn(game);
        });

        assert!(
            ai_pieces >= passive_pieces,
            "AI locked {ai_pieces} pieces but passive locked {passive_pieces}; \
             AI should do at least as well"
        );
    }

    /// The AI should clear at least some lines over a run of many pieces
    /// (on a known seed where lines are reasonably achievable).
    #[test]
    fn ai_clears_lines() {
        const SEED: u64 = 123;

        let mut ernie = Computer::new();
        let (_, total_lines) = run_game(Game::new(SEED), |game| {
            ernie.take_turn(game);
        });

        assert!(
            total_lines > 0,
            "AI should clear at least one line, but cleared {total_lines}"
        );
    }

    // --- Oracle: AI heuristic penalty weights (BTComputer.C:39-54) -----------

    /// Pin the board-eval penalty/bonus weights to the original's `levels`-time
    /// `BTCPenalties` (set in `BTComputer::BTComputer`, BTComputer.C:45-54). The
    /// eval *structure* here is a faithful-in-spirit approximation, but these
    /// weights are the exact numbers the 1994 AI used — drift in them silently
    /// changes how Ernie plays.
    #[test]
    fn eval_penalty_weights_match_btcomputer() {
        assert_eq!(OPEN_HOLE_PENALTY, 7_000.0, "BT_OPEN_HOLE_PENALTY (BTComputer.C:45)");
        assert_eq!(CLOSED_HOLE_PENALTY, 10_000.0, "BT_CLOSED_HOLE_PENALTY (BTComputer.C:46)");
        assert_eq!(HEIGHT_PENALTY, 30_000.0, "BT_HEIGHT_PENALTY (BTComputer.C:48)");
        assert_eq!(COVERED_HOLE_PENALTY, 3_000.0, "BT_COVERED_HOLE_PENALTY (BTComputer.C:50)");
        assert_eq!(LINE_BONUS, 5_000.0, "BT_LINE_BONUS (BTComputer.C:51)");
        assert_eq!(VARIANCE_PENALTY, 50.0, "BT_VARIANCE_PENALTY (BTComputer.C:54)");
        assert_eq!(MIDLINE, 14, "BT_MIDLINE (BTComputer.C:39)");
    }

    /// Ernie's difficulty ladder — the per-move delays (ms) from
    /// `BTComputer.C:86-102` `levels[]`, Comatose (4000) … Bionic (0).
    #[test]
    fn difficulty_ladder_matches_btcomputer() {
        assert_eq!(
            AI_LEVELS,
            [4000, 3000, 2000, 1500, 1250, 1000, 750, 550, 400, 350, 300, 225, 100, 10, 0],
            "levels[].timeout (BTComputer.C:86-102)"
        );
    }
}
