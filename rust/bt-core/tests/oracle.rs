//! Oracle tests — pin the engine's *values* to the original 1994 C++ reference
//! in `usr/src/`, each assertion carrying the `file:line` it mirrors.
//!
//! These are the antidote to the Ernie-scoring bug class: every test here
//! asserts a concrete number the original produces, not merely that the engine
//! "works". A faithful port that quietly drifts to a plausible-but-wrong value
//! (28 vs 14, value*lines vs value+lines, bazaar at 25 vs 20) fails loudly here.
//!
//! When one of these fails, the question is always "did the original really do
//! this?" — go read the cited line, don't just update the constant.

use bt_core::constants::*;
use bt_core::{Board, Cell, Game};

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Human hard-drop bonus is `BT_BOARD_HGT - y_` — the further the piece still
/// had to fall, the more it's worth. `BTGame::beginDrop`, BTGame.C:729:
///     score_manager_->rep_.score_ += BT_BOARD_HGT - y_;
#[test]
fn human_hard_drop_bonus_is_board_height_minus_y() {
    let mut g = Game::new(1);
    let (_, y0) = g.piece_pos();
    assert_eq!(g.score().score, 0, "no score before the drop");

    g.begin_drop();
    assert_eq!(
        g.score().score,
        (BT_BOARD_HGT - y0) as i64,
        "first drop from spawn y={y0} must bank BT_BOARD_HGT - y (BTGame.C:729)"
    );
}

/// The same bonus is height-*dependent*: a piece soft-dropped lower banks less.
/// This is the human curve Ernie must NOT be on (see the AI oracle).
#[test]
fn human_hard_drop_bonus_shrinks_with_depth() {
    let mut g = Game::new(1);
    for _ in 0..3 {
        g.soft_drop(); // soft drop is score-free; it only changes y
    }
    let (_, y) = g.piece_pos();
    assert_eq!(g.score().score, 0, "soft drop must not score");

    g.begin_drop();
    assert_eq!(
        g.score().score,
        (BT_BOARD_HGT - y) as i64,
        "after dropping to y={y}, the bonus is BT_BOARD_HGT - y (BTGame.C:729)"
    );
    assert!(
        (BT_BOARD_HGT - y) < BT_BOARD_HGT,
        "a lower piece banks strictly less than a spawn-height drop"
    );
}

/// Ernie's placement score is a *flat* `BT_BOARD_HGT / 2` per piece, regardless
/// of drop height — NOT the human bonus. `BTComputer::run`, BTComputer.C:1255:
///     score_manager_->rep_.score_ += BT_BOARD_HGT / 2;
#[test]
fn ai_placement_score_is_flat_half_board_height() {
    // At spawn height.
    let mut g = Game::new(1);
    g.ai_begin_drop();
    assert_eq!(
        g.score().score,
        (BT_BOARD_HGT / 2) as i64,
        "Ernie banks a flat BT_BOARD_HGT/2 (BTComputer.C:1255)"
    );

    // ...and the SAME flat value three rows lower — height-independent.
    let mut g2 = Game::new(1);
    for _ in 0..3 {
        g2.soft_drop();
    }
    g2.ai_begin_drop();
    assert_eq!(
        g2.score().score,
        (BT_BOARD_HGT / 2) as i64,
        "Ernie's flat score does not depend on y, unlike the human bonus"
    );
}

// ---------------------------------------------------------------------------
// Line-clear rewards
// ---------------------------------------------------------------------------

/// Funds for a clear are `value * lines` — multiplicative, no additive bonus.
/// `BTBoardManager::checkLines`, BTBoardManager.C:613:
///     short funds = value * lines.inc();
#[test]
fn line_clear_funds_are_value_times_lines() {
    // One full row of plain colored boxes: value 0 -> funds 0.
    let mut b = Board::standard(false);
    let w = b.width;
    let bottom = b.height - 1;
    for x in 0..w {
        b.set(x, bottom, Some(Cell::color(2)));
    }
    let lc = b.check_lines();
    assert_eq!(lc.lines, 1);
    assert_eq!(lc.value, 0, "plain color boxes are worth 0 (BTBox.H)");
    assert_eq!(lc.funds, 0, "0 * 1 = 0");

    // One full row carrying a single die(6): value 6, funds = 6 * 1.
    let mut b = Board::standard(false);
    for x in 0..w {
        b.set(x, bottom, Some(Cell::color(2)));
    }
    b.set(0, bottom, Some(Cell::die(6)));
    let lc = b.check_lines();
    assert_eq!(lc.lines, 1);
    assert_eq!(lc.value, 6);
    assert_eq!(lc.funds, 6, "value(6) * lines(1)");

    // Two simultaneous full rows, total value 9 across both: funds = 9 * 2 = 18.
    let mut b = Board::standard(false);
    for x in 0..w {
        b.set(x, bottom, Some(Cell::color(2)));
        b.set(x, bottom - 1, Some(Cell::color(2)));
    }
    b.set(0, bottom, Some(Cell::die(4))); // +4
    b.set(1, bottom - 1, Some(Cell::die(5))); // +5
    let lc = b.check_lines();
    assert_eq!(lc.lines, 2);
    assert_eq!(lc.value, 9);
    assert_eq!(lc.funds, 18, "value(9) * lines(2) — multiplicative, not 9+2");
}

/// NiceDay's gift: clearing a line containing an un-landed happy face banks its
/// 150 (BT_HAPPY_VAL). NiceDay forces a happy piece (piece_manager); placed and
/// cleared, that cell's 150 flows into `value`, hence funds = value * lines.
/// This pins the gameplay funds path end-to-end.
#[test]
fn happy_face_in_a_cleared_line_banks_150() {
    let mut b = Board::standard(false);
    let w = b.width;
    let bottom = b.height - 1;
    for x in 0..w {
        b.set(x, bottom, Some(Cell::color(2)));
    }
    b.set(0, bottom, Some(Cell::happy()));

    let lc = b.check_lines();
    assert_eq!(lc.lines, 1);
    assert_eq!(lc.value, BT_HAPPY_VAL, "the happy face contributes 150");
    assert_eq!(lc.funds, BT_HAPPY_VAL, "150 * 1 line");
}

/// Per-box `value()` constants, from the `BTBox` subclasses (BTBox.H):
/// plain box = 0, die = its pips (1..=6), un-landed happy = BT_HAPPY_VAL (150).
#[test]
fn box_values_match_btbox() {
    assert_eq!(Cell::color(3).value(), 0, "BTBox::value() = 0");
    for pips in 1..=6u8 {
        assert_eq!(Cell::die(pips).value(), pips as i32, "BTDieBox::value() = pips");
    }
    assert_eq!(Cell::happy().value(), BT_HAPPY_VAL, "BTHappyBox::value() = 150");
    assert_eq!(BT_HAPPY_VAL, 150, "BT_HAPPY_VAL literal");
}

// ---------------------------------------------------------------------------
// Bazaar trigger
// ---------------------------------------------------------------------------

/// The bazaar opens every 20 combined (self + opponent) lines.
/// `#define BT_LINES_TIL_BAZ 20` (BTScoreManager.C:15); the countdown wraps as
/// `20 - ((my_lines + op_lines) % 20)` (BTScoreManager.C:170-176).
#[test]
fn bazaar_trigger_is_twenty_combined_lines() {
    assert_eq!(BT_LINES_TIL_BAZ, 20, "BT_LINES_TIL_BAZ (BTScoreManager.C:15)");

    let mut g = Game::new(1);
    assert_eq!(
        g.lines_til_bazaar(),
        BT_LINES_TIL_BAZ,
        "a fresh game counts down from 20"
    );

    // The opponent clears lines one at a time; the countdown ticks down and the
    // bazaar opens exactly when combined lines first reaches 20 (the wrap).
    for n in 1..20 {
        g.receive_op_score(0, n, 0);
        assert!(!g.is_in_bazaar(), "bazaar stays shut at {n} combined lines");
        assert_eq!(g.lines_til_bazaar(), (BT_LINES_TIL_BAZ as i64 - n) as i32);
    }
    g.receive_op_score(0, 20, 0);
    assert!(
        g.is_in_bazaar(),
        "20 combined lines opens the bazaar (BTScoreManager.C:170-176)"
    );
}

// ---------------------------------------------------------------------------
// Geometry & timing constants (BTConstants.H)
// ---------------------------------------------------------------------------

/// Board / piece geometry, BTConstants.H:89-102.
#[test]
fn board_geometry_matches_btconstants() {
    assert_eq!(BT_BOARD_WTH, 10, "BTConstants.H:89");
    assert_eq!(BT_BOARD_HGT, 28, "BTConstants.H:90");
    assert_eq!(BT_DEFAULT_X, 5, "BTConstants.H:98");
    assert_eq!(BT_DEFAULT_Y, 0, "BTConstants.H:99");
    assert_eq!(BT_PIECE_WIDTH, 8, "BTConstants.H:101");
    assert_eq!(BT_PIECE_HEIGHT, 8, "BTConstants.H:102");

    let b = Board::standard(false);
    assert_eq!((b.width, b.height), (10, 28), "a standard board is 10x28");
}

/// Drop / slide cadence, BTConstants.H:92-94.
#[test]
fn drop_timing_matches_btconstants() {
    assert_eq!(BT_FAST_DROP_TIME, 10, "BTConstants.H:92");
    assert_eq!(BT_DROP_TIME, 512, "BTConstants.H:93");
    assert_eq!(BT_SLIDE_TIME, 150, "BTConstants.H:94");
}
