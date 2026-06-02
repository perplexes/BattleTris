//! Integration tests for the board manager (`BTBoardManager`) port: line
//! clearing + funds economy, idiot detection, FALL_OUT collision, and a full
//! piece drop-and-land through the public API.

use bt_core::constants::*;
use bt_core::{Board, Cell, Piece, PieceKind, WeaponToken};

/// Fill an entire row with plain colored boxes.
fn fill_row(b: &mut Board, y: i32, color: i32) {
    for x in 0..b.width {
        b.set(x, y, Some(Cell::color(color)));
    }
}

#[test]
fn single_line_of_color_clears_with_zero_funds() {
    let mut b = Board::standard(false);
    fill_row(&mut b, BT_BOARD_HGT - 1, BT_RED);
    let r = b.check_lines();
    assert_eq!(r.lines, 1);
    assert_eq!(r.value, 0);
    assert_eq!(r.funds, 0, "plain colored boxes carry no funds value");
    for x in 0..b.width {
        assert!(b.get(x, BT_BOARD_HGT - 1).is_none(), "row should be cleared");
    }
}

#[test]
fn die_in_line_awards_its_pips() {
    let mut b = Board::standard(false);
    fill_row(&mut b, BT_BOARD_HGT - 1, BT_RED);
    b.set(3, BT_BOARD_HGT - 1, Some(Cell::die(4)));
    let r = b.check_lines();
    // funds = value * lines = 4 * 1
    assert_eq!(r.lines, 1);
    assert_eq!(r.value, 4);
    assert_eq!(r.funds, 4);
}

#[test]
fn double_clear_multiplies_pip_total_by_line_count() {
    let mut b = Board::standard(false);
    let bottom = BT_BOARD_HGT - 1;
    fill_row(&mut b, bottom, BT_RED);
    fill_row(&mut b, bottom - 1, BT_BLUE);
    b.set(3, bottom, Some(Cell::die(3)));
    b.set(7, bottom - 1, Some(Cell::die(2)));
    let r = b.check_lines();
    // value = 3 + 2 = 5, lines = 2 => funds = 10  ("a double earns twice")
    assert_eq!(r.lines, 2);
    assert_eq!(r.value, 5);
    assert_eq!(r.funds, 10);
    // board fully cleared
    for y in 0..BT_BOARD_HGT {
        for x in 0..BT_BOARD_WTH {
            assert!(b.get(x, y).is_none());
        }
    }
}

#[test]
fn happy_face_awards_150_when_cleared() {
    let mut b = Board::standard(false);
    let bottom = BT_BOARD_HGT - 1;
    fill_row(&mut b, bottom, BT_RED);
    b.set(5, bottom, Some(Cell::happy()));
    let r = b.check_lines();
    assert_eq!(r.lines, 1);
    assert_eq!(r.value, BT_HAPPY_VAL);
    assert_eq!(r.funds, BT_HAPPY_VAL);
}

#[test]
fn happy_face_missed_turns_to_frown_and_flags_idiot() {
    let mut b = Board::standard(false);
    let bottom = BT_BOARD_HGT - 1;
    // A happy face alone on a non-full row: it should "land" (frown) and flag.
    b.set(5, bottom, Some(Cell::happy()));
    let r = b.check_lines();
    assert_eq!(r.lines, 0);
    let cell = b.get(5, bottom).expect("happy still present");
    assert_eq!(cell.value(), 0, "missed smiley becomes a frown worth 0");
    assert_eq!(b.flush_idiot(), Some(BT_MISSED_SMILEY));
}

#[test]
fn near_death_when_stack_reaches_top() {
    let mut b = Board::standard(false);
    // A single tall column from row 7 to the bottom: no full lines, but the
    // stack reaches within 8 of the top => near-death.
    for y in 7..BT_BOARD_HGT {
        b.set(0, y, Some(Cell::color(BT_GREEN)));
    }
    let r = b.check_lines();
    assert_eq!(r.lines, 0);
    assert_eq!(b.flush_idiot(), Some(BT_NEAR_DEATH));
}

#[test]
fn occupied_respects_bounds() {
    let b = Board::standard(false);
    assert!(b.occupied(-1, 5));
    assert!(b.occupied(BT_BOARD_WTH, 5));
    assert!(b.occupied(5, -1));
    assert!(b.occupied(5, BT_BOARD_HGT));
    assert!(!b.occupied(5, 5));
}

#[test]
fn fall_out_opens_the_middle_floor_but_keeps_the_ledges() {
    let mut b = Board::standard(false);
    b.active.activate(WeaponToken::FallOut);
    let below_floor = BT_BOARD_HGT; // y == height, just under the floor
    // Ledge columns stay solid...
    assert!(b.occupied(0, below_floor));
    assert!(b.occupied(BT_BOARD_WTH - 1, below_floor));
    // ...the middle is open (a piece can fall through).
    assert!(!b.occupied(BT_FALL_OUT_LEDGE, below_floor));
    assert!(!b.occupied(BT_BOARD_WTH - BT_FALL_OUT_LEDGE - 1, below_floor));
}

#[test]
fn box_piece_drops_to_the_floor_and_lands() {
    let mut b = Board::standard(false);
    // Box has local cells (1,1)(1,2)(2,1)(2,2); spawn x=4 so it sits at cols 5,6.
    let mut p = Piece::construct(PieceKind::Box, 4, 0, 0);
    let mut y = 0;
    while p.move_to(&b, p.x, y + 1) {
        y += 1;
    }
    p.land(&mut b);

    // Bottom-most occupied cells are local y=2 => board row 27; the others 26.
    let bottom = BT_BOARD_HGT - 1;
    assert!(b.get(5, bottom).is_some());
    assert!(b.get(6, bottom).is_some());
    assert!(b.get(5, bottom - 1).is_some());
    assert!(b.get(6, bottom - 1).is_some());

    let filled: usize = (0..BT_BOARD_HGT)
        .flat_map(|yy| (0..BT_BOARD_WTH).map(move |xx| (xx, yy)))
        .filter(|&(xx, yy)| b.get(xx, yy).is_some())
        .count();
    assert_eq!(filled, 4, "exactly the 4 box cells should be on the board");
}
