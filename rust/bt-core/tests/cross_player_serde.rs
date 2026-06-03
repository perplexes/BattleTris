//! Round-trip tests for the cross-player board/arsenal codec that online Swap,
//! Lazy Susan, and the spies ship over the data channel. The live 2-peer
//! transport isn't testable headlessly, but the encoding that travels over it
//! is — and that's where the faithfulness (die values, gimp, hidden cells,
//! arsenal quantities) lives.

use bt_core::weapons::WeaponToken;
use bt_core::{Cell, Game};

#[test]
fn board_round_trips_through_export_import() {
    let mut a = Game::new(1);
    a.board_mut().set(0, 27, Some(Cell::die(6))); // value-bearing
    a.board_mut().set(5, 14, Some(Cell::structure())); // non-removable
    a.board_mut().set(9, 0, Some(Cell::gimp(3))); // gimp carries a value
    let mut hidden = Cell::color(4);
    hidden.hide();
    a.board_mut().set(3, 10, Some(hidden)); // twilight/hidden

    let data = a.export_board();
    let mut b = Game::new(999); // a different board to overwrite
    b.import_board(&data);

    for y in 0..28 {
        for x in 0..10 {
            let ca = a.board().get(x, y).map(|c| (c.id(), c.value(), c.hidden));
            let cb = b.board().get(x, y).map(|c| (c.id(), c.value(), c.hidden));
            assert_eq!(ca, cb, "cell ({x},{y}) diverged across the codec");
        }
    }
}

#[test]
fn import_board_ignores_wrong_length() {
    let mut g = Game::new(1);
    g.board_mut().set(0, 27, Some(Cell::die(5)));
    g.import_board(&[1, 2, 3]); // garbage length -> no-op
    assert_eq!(g.board().get(0, 27).map(|c| c.value()), Some(5), "board untouched");
}

#[test]
fn arsenal_round_trips_through_export_import() {
    let mut a = Game::new(1);
    a.grant_weapon(WeaponToken::RiseUp);
    a.grant_weapon(WeaponToken::RiseUp);
    a.grant_weapon(WeaponToken::Blind);

    let mut b = Game::new(2);
    b.import_arsenal(&a.export_arsenal());

    assert_eq!(b.arsenal_token(0), WeaponToken::RiseUp.index() as i32);
    assert_eq!(b.arsenal_quantity(0), 2, "quantity preserved");
    assert_eq!(b.arsenal_token(1), WeaponToken::Blind.index() as i32);
    assert_eq!(b.arsenal_quantity(1), 1);
}

/// Slot positions (including a hole) are preserved across import — rebuilding
/// via `buy` used to compact holes and shift later weapons up a slot.
#[test]
fn arsenal_import_preserves_slot_positions() {
    let mut data = vec![0i32; 20];
    for i in 0..10 {
        data[i * 2] = -1; // empty
    }
    data[0] = WeaponToken::RiseUp.index() as i32; // slot 0
    data[1] = 2;
    // slot 1 is a hole
    data[4] = WeaponToken::Gimp.index() as i32; // slot 2
    data[5] = 3;

    let mut g = Game::new(1);
    g.import_arsenal(&data);

    assert_eq!(g.arsenal_token(0), WeaponToken::RiseUp.index() as i32);
    assert_eq!(g.arsenal_quantity(0), 2);
    assert_eq!(g.arsenal_token(1), -1, "the hole at slot 1 is preserved");
    assert_eq!(g.arsenal_token(2), WeaponToken::Gimp.index() as i32, "Gimp stays at slot 2");
    assert_eq!(g.arsenal_quantity(2), 3);
}

/// A hostile peer quantity is clamped, not looped — no hang, no overflow.
#[test]
fn arsenal_import_clamps_hostile_quantity() {
    let mut data = vec![0i32; 20];
    for i in 0..10 {
        data[i * 2] = -1;
    }
    data[0] = WeaponToken::RiseUp.index() as i32;
    data[1] = i32::MAX;

    let mut g = Game::new(1);
    g.import_arsenal(&data); // must return promptly

    assert_eq!(g.arsenal_token(0), WeaponToken::RiseUp.index() as i32);
    assert_eq!(g.arsenal_quantity(0), u16::MAX, "clamped to u16::MAX");
}
