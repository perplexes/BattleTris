//! Per-weapon oracle — board-level effects.
//!
//! Each weapon is unique, so there's no single assertion shape; this file pins
//! the *board mutation* of every weapon whose effect lands on the grid, by
//! driving `Board::apply_weapon` directly (the same call `apply_weapon_on`
//! makes, after `set_active`). Game-parameter weapons (Speedy/Meadow/Upbyside
//! controls/NoSlide/Carter/Keating/Reagan/Mondale) live in `weapons_game.rs`;
//! piece-stream weapons (NoDice/FearedWeird/FourByFour/SoLong/Broken/NiceDay)
//! live in `piece_manager`'s unit tests; cross-player weapons (Swap/Susan/
//! Mirror/spies) are exercised by the interaction + fuzz layers.
//!
//! References are to `usr/src/game/BTBoardManager.C` unless noted.

use bt_core::cell::CellKind;
use bt_core::constants::*;
use bt_core::rng::Rng;
use bt_core::weapons::WeaponToken;
use bt_core::{Board, Cell};

/// Apply a weapon to the board exactly as `Game::apply_weapon_on` does:
/// set the active flag, then run the one-shot mutation.
fn apply(b: &mut Board, tok: WeaponToken, rng: &mut Rng) {
    b.set_active(tok, true);
    b.apply_weapon(tok, rng);
}

fn count(b: &Board) -> usize {
    (0..b.height)
        .flat_map(|y| (0..b.width).map(move |x| (x, y)))
        .filter(|&(x, y)| b.get(x, y).is_some())
        .count()
}

fn fill_rect(b: &mut Board, x0: i32, x1: i32, y0: i32, y1: i32, cell: Cell) {
    for y in y0..y1 {
        for x in x0..x1 {
            b.set(x, y, Some(cell));
        }
    }
}

/// Upbyside-down: flip the board top↔bottom and latch `upside`.
/// `BTBoardManager::flipOnHoriz`; flag at board.rs apply_weapon.
#[test]
fn upbyside_flips_board_top_to_bottom() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(1);
    b.set(3, 0, Some(Cell::die(5))); // a marker near the top
    assert!(!b.upside);

    apply(&mut b, WeaponToken::Upbyside, &mut rng);

    assert!(b.upside, "Upbyside latches the upside flag");
    assert!(b.get(3, 0).is_none(), "the top cell moved");
    assert_eq!(
        b.get(3, b.height - 1).map(|c| c.value()),
        Some(5),
        "it's now mirrored to the bottom row (flipOnHoriz)"
    );
}

/// Flip Out: mirror left↔right (`BTBoardManager::flipOnVert`).
#[test]
fn flip_out_mirrors_left_to_right() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(1);
    b.set(0, 5, Some(Cell::die(4)));

    apply(&mut b, WeaponToken::FlipOut, &mut rng);

    assert!(b.get(0, 5).is_none());
    assert_eq!(
        b.get(b.width - 1, 5).map(|c| c.value()),
        Some(4),
        "leftmost column mirrors to the rightmost"
    );
}

/// Fallout: the middle columns drain away, the side ledges stay.
#[test]
fn fallout_drains_the_middle_columns() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(1);
    let (w, h) = (b.width, b.height);
    fill_rect(&mut b, 0, w, 0, h, Cell::color(1));

    apply(&mut b, WeaponToken::FallOut, &mut rng);

    // Non-ledge columns [LEDGE, w-LEDGE) are emptied; the ledges remain.
    for x in BT_FALL_OUT_LEDGE..(b.width - BT_FALL_OUT_LEDGE) {
        for y in 0..b.height {
            assert!(b.get(x, y).is_none(), "middle column {x} should have fallen out");
        }
    }
    for &x in &[0, 1, b.width - 2, b.width - 1] {
        assert!(b.get(x, b.height - 1).is_some(), "ledge column {x} stays filled");
    }
}

/// Missing Pieces: remove exactly one removable box.
#[test]
fn missing_removes_exactly_one_box() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(7);
    let (w, h) = (b.width, b.height);
    fill_rect(&mut b, 0, w, h - 3, h, Cell::color(1));
    let before = count(&b);

    apply(&mut b, WeaponToken::Missing, &mut rng);

    assert_eq!(count(&b), before - 1, "Missing removes exactly one block");
}

/// Piece It Together: add one box in the middle two quarters.
#[test]
fn piece_it_adds_one_box_in_the_middle() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(7);

    apply(&mut b, WeaponToken::PieceIt, &mut rng);

    assert_eq!(count(&b), 1, "PieceIt adds exactly one block");
    // It must land in the middle two quarters: y in [h/4, h/4 + h/2).
    let (lo, hi) = (b.height / 4, b.height / 4 + b.height / 2);
    let placed = (0..b.height)
        .flat_map(|y| (0..b.width).map(move |x| (x, y)))
        .find(|&(x, y)| b.get(x, y).is_some())
        .unwrap();
    assert!((lo..hi).contains(&placed.1), "placed at y={} outside [{lo},{hi})", placed.1);
}

/// Bug Report: like PieceIt, but the block is invisible (`BT_INVISIBLE`).
#[test]
fn bug_adds_one_invisible_box() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(7);

    apply(&mut b, WeaponToken::Bug, &mut rng);

    assert_eq!(count(&b), 1);
    let cell = (0..b.height)
        .flat_map(|y| (0..b.width).map(move |x| (x, y)))
        .find_map(|(x, y)| b.get(x, y))
        .unwrap();
    assert_eq!(cell.kind, CellKind::Color(BT_INVISIBLE), "Bug plants an invisible box");
}

/// The Blind Cleric: bomb roughly half the removable boxes.
#[test]
fn blind_removes_a_chunk_of_boxes() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(7);
    let (w, h) = (b.width, b.height);
    fill_rect(&mut b, 0, w, h - 10, h, Cell::color(1));
    let before = count(&b); // 100

    apply(&mut b, WeaponToken::Blind, &mut rng);

    let after = count(&b);
    assert!(after < before, "Blind removes blocks");
    assert!(after > 0, "Blind doesn't clear the whole board");
    // ~50% each, so a wide sanity band (P of leaving the band is astronomically low).
    assert!((before / 5..before * 4 / 5).contains(&after), "expected ~half removed, got {after}/{before}");
}

/// The Gimp: every removable box becomes a gimp box of the same value.
#[test]
fn gimp_converts_boxes_preserving_value() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(1);
    b.set(0, 27, Some(Cell::color(2))); // value 0
    b.set(1, 27, Some(Cell::die(6))); // value 6
    let before = count(&b);

    apply(&mut b, WeaponToken::Gimp, &mut rng);

    assert_eq!(count(&b), before, "Gimp transforms in place, count unchanged");
    assert_eq!(b.get(0, 27).map(|c| c.kind), Some(CellKind::Gimp(0)));
    assert_eq!(b.get(1, 27).map(|c| c.kind), Some(CellKind::Gimp(6)), "value carried through");
}

/// The Twilight Zone: every box becomes invisible (`hidden`, id == -1), but
/// otherwise unchanged (still present, still worth its value).
#[test]
fn twilight_hides_every_box() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(1);
    let (w, h) = (b.width, b.height);
    fill_rect(&mut b, 0, w, h - 2, h, Cell::die(3));
    let before = count(&b);

    apply(&mut b, WeaponToken::Twilight, &mut rng);

    assert_eq!(count(&b), before, "Twilight hides, doesn't remove");
    for y in (b.height - 2)..b.height {
        for x in 0..b.width {
            let c = b.get(x, y).unwrap();
            assert!(c.hidden, "every box is hidden");
            assert_eq!(c.id(), -1, "a hidden box renders as id -1");
            assert_eq!(c.value(), 3, "but still scores its value when cleared");
        }
    }
}

/// Bottle neck: structure boxes wall off the sides of the central belt.
#[test]
fn bottle_walls_off_the_neck() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(1);

    apply(&mut b, WeaponToken::Bottle, &mut rng);

    let h = BT_BOARD_HGT;
    for y in (h / 2 - BT_BOTTLE_Y)..(h / 2 + BT_BOTTLE_Y) {
        for x in 0..BT_BOTTLE_X {
            assert_eq!(b.get(x, y).map(|c| c.kind), Some(CellKind::Structure), "left wall at ({x},{y})");
            let rx = b.width - x - 1;
            assert_eq!(b.get(rx, y).map(|c| c.kind), Some(CellKind::Structure), "right wall at ({rx},{y})");
        }
    }
    assert!(b.get(b.width / 2, h / 2).is_none(), "the neck itself stays open");
    // Structure boxes resist removal.
    assert!(!b.get(0, h / 2).unwrap().is_removable());
}

/// Rise Up: a solid garbage row with exactly one gap pushes the stack up.
#[test]
fn rise_up_inserts_a_solid_row_with_one_gap() {
    let mut b = Board::standard(false);
    let mut rng = Rng::new(3);

    apply(&mut b, WeaponToken::RiseUp, &mut rng);

    let bottom = b.height - 1;
    let filled = (0..b.width).filter(|&x| b.get(x, bottom).is_some()).count();
    assert_eq!(filled, (b.width - 1) as usize, "all columns but one are filled");
    // The garbage is green and worth nothing.
    let g = (0..b.width).find_map(|x| b.get(x, bottom)).unwrap();
    assert_eq!(g.id(), BT_GREEN);
    assert_eq!(g.value(), 0, "garbage carries no funds");
}

/// The Force: a cleared line vanishes but the board does NOT cascade.
/// `removeLine` honors FORCE (board.rs): clear the row in place, no shift.
#[test]
fn force_clears_without_cascading() {
    let mut b = Board::standard(false);
    b.set_active(WeaponToken::Force, true);
    let bottom = b.height - 1;
    for x in 0..b.width {
        b.set(x, bottom, Some(Cell::color(1)));
    }
    b.set(0, bottom - 1, Some(Cell::color(1))); // a lone block sitting above

    let lc = b.check_lines();

    assert_eq!(lc.lines, 1, "the full row still counts as cleared");
    assert!(b.get(0, bottom).is_none(), "the cleared row is empty");
    assert!(
        b.get(0, bottom - 1).is_some(),
        "Force suppresses gravity — the block above stays put"
    );
}
