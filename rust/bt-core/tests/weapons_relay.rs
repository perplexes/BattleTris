//! Integration tests for the weapon system + two-player relay surface:
//! a launched weapon arriving as WPN_ON at the next lock, and the bazaar
//! trigger driven by combined (self + opponent) lines.

use bt_core::constants::*;
use bt_core::game::GameEvent;
use bt_core::{Game, WeaponToken};

/// Fast-drop pieces until one locks (a `Locked` event is emitted).
fn drop_one_piece(g: &mut Game) {
    for _ in 0..4000 {
        g.begin_drop();
        g.tick(50);
        if g
            .take_events()
            .iter()
            .any(|e| matches!(e, GameEvent::Locked { .. }))
        {
            return;
        }
        if g.is_game_over() {
            return;
        }
    }
    panic!("piece never locked");
}

/// Max filled cells in any single row.
fn densest_row(g: &Game) -> i32 {
    let b = g.board();
    (0..b.height)
        .map(|y| (0..b.width).filter(|&x| b.get(x, y).is_some()).count() as i32)
        .max()
        .unwrap_or(0)
}

#[test]
fn received_rise_up_inserts_a_garbage_line_at_next_lock() {
    let mut g = Game::new(123);
    // Opponent launched Rise Up at us; it queues until our next piece lock.
    g.receive_weapon(WeaponToken::RiseUp);
    assert!(densest_row(&g) < 9, "no garbage line yet");
    drop_one_piece(&mut g);
    // Rise Up inserts a solid row with a single hole => a row of 9 cells.
    assert!(
        densest_row(&g) >= 9,
        "a garbage line (9 filled cells) should have been inserted"
    );
}

#[test]
fn bazaar_triggers_on_combined_twenty_lines() {
    let mut g = Game::new(7);
    assert!(!g.is_in_bazaar());
    assert_eq!(g.lines_til_bazaar(), BT_LINES_TIL_BAZ);

    // Opponent reports 19 lines: countdown drops to 1, no bazaar yet.
    g.receive_op_score(0, 19, 0);
    let _ = g.take_events();
    assert_eq!(g.lines_til_bazaar(), 1);
    assert!(!g.is_in_bazaar());

    // Opponent crosses 20 combined lines: bazaar opens.
    g.receive_op_score(0, 20, 0);
    assert!(g.is_in_bazaar());
    assert!(g
        .take_events()
        .iter()
        .any(|e| matches!(e, GameEvent::EnterBazaar)));
}

#[test]
fn bazaar_freezes_the_clock() {
    let mut g = Game::new(11);
    // Open the bazaar the way it really happens: the countdown must first drop
    // below 20, then crossing a multiple of 20 fires it.
    g.receive_op_score(0, 19, 0);
    g.receive_op_score(0, 20, 0);
    assert!(g.is_in_bazaar());
    let _ = g.take_events();
    let before = g.piece_pos();
    for _ in 0..50 {
        g.tick(BT_DROP_TIME);
    }
    assert_eq!(g.piece_pos(), before, "no falling while in the bazaar");
    g.leave_bazaar();
    g.tick(BT_DROP_TIME);
    assert_eq!(g.piece_pos().1, before.1 + 1, "play resumes after leaving");
}
