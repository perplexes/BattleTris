//! Integration tests for the deterministic game loop (`BTGame` port).

use bt_core::constants::*;
use bt_core::game::GameEvent;
use bt_core::Game;

/// Snapshot the board as a comparable vector of (id, value) per square.
fn snapshot(g: &Game) -> Vec<Option<(i32, i32)>> {
    let b = g.board();
    (0..BT_BOARD_HGT)
        .flat_map(|y| (0..BT_BOARD_WTH).map(move |x| (x, y)))
        .map(|(x, y)| b.get(x, y).map(|c| (c.id(), c.value())))
        .collect()
}

#[test]
fn new_game_spawns_a_piece() {
    let g = Game::new(1);
    assert!(g.current_piece().is_some());
    assert!(!g.is_game_over());
    assert_eq!(g.piece_pos(), (g.piece_pos().0, 0)); // spawns at top (y == 0)
}

#[test]
fn piece_falls_one_row_per_drop_interval() {
    let mut g = Game::new(1);
    let (_, y0) = g.piece_pos();
    assert_eq!(y0, 0);
    g.tick(BT_DROP_TIME);
    let (_, y1) = g.piece_pos();
    assert_eq!(y1, 1, "one drop interval should move the piece down one row");
}

#[test]
fn piece_locks_and_fills_the_board() {
    let mut g = Game::new(7);
    // Fast-forward until the first lock.
    let mut locked = false;
    for _ in 0..2000 {
        g.tick(BT_DROP_TIME);
        if g
            .take_events()
            .iter()
            .any(|e| matches!(e, GameEvent::Locked { .. }))
        {
            locked = true;
            break;
        }
    }
    assert!(locked, "a piece should lock within a reasonable time");
    let filled = snapshot(&g).iter().filter(|c| c.is_some()).count();
    assert!(filled > 0, "the locked piece should leave cells on the board");
}

#[test]
fn game_is_deterministic_for_a_fixed_seed_and_input() {
    let mut a = Game::new(42);
    let mut b = Game::new(42);
    for _ in 0..5000 {
        a.tick(BT_DROP_TIME);
        b.tick(BT_DROP_TIME);
    }
    assert_eq!(snapshot(&a), snapshot(&b), "boards must match");
    assert_eq!(a.score(), b.score(), "scores must match");
    assert_eq!(a.is_game_over(), b.is_game_over());
}

#[test]
fn different_seeds_diverge() {
    let mut a = Game::new(1);
    let mut b = Game::new(2);
    for _ in 0..400 {
        a.tick(BT_DROP_TIME);
        b.tick(BT_DROP_TIME);
    }
    assert_ne!(
        snapshot(&a),
        snapshot(&b),
        "independent seeds should produce different boards"
    );
}

#[test]
fn game_eventually_tops_out_with_no_input() {
    let mut g = Game::new(3);
    let mut over = false;
    for _ in 0..200_000 {
        g.tick(BT_DROP_TIME);
        if g.is_game_over() {
            over = true;
            break;
        }
    }
    assert!(over, "stacking pieces with no clears should eventually top out");
    // After game over, ticks are inert and the piece is gone.
    let snap = snapshot(&g);
    g.tick(BT_DROP_TIME);
    assert_eq!(snapshot(&g), snap, "no state changes after game over");
}

#[test]
fn begin_drop_awards_height_bonus() {
    let mut g = Game::new(5);
    assert_eq!(g.score().score, 0);
    let (_, y) = g.piece_pos();
    g.begin_drop();
    assert_eq!(
        g.score().score,
        (BT_BOARD_HGT - y) as i64,
        "hard drop awards BT_BOARD_HGT - y"
    );
}

#[test]
fn pause_freezes_the_clock() {
    let mut g = Game::new(9);
    g.set_paused(true);
    let before = snapshot(&g);
    let pos_before = g.piece_pos();
    for _ in 0..100 {
        g.tick(BT_DROP_TIME);
    }
    assert_eq!(snapshot(&g), before, "paused board must not change");
    assert_eq!(g.piece_pos(), pos_before, "paused piece must not move");

    g.set_paused(false);
    g.tick(BT_DROP_TIME);
    assert_eq!(g.piece_pos().1, pos_before.1 + 1, "unpause resumes falling");
}

#[test]
fn lines_and_funds_are_non_negative_over_a_full_game() {
    let mut g = Game::new(123);
    for _ in 0..200_000 {
        g.tick(BT_DROP_TIME);
        if g.is_game_over() {
            break;
        }
    }
    let s = g.score();
    assert!(s.lines >= 0 && s.funds >= 0 && s.score >= 0);
}
