//! Weapons layer 2 — interactions.
//!
//! NOTE: Swap / Susan / Mirror ARE now implemented (engine + VsComputer relay
//! in bt-ai/src/vs.rs, online in bt-wasm/www/main.js). Their interaction tests
//! live with the relay logic (bt-ai/src/vs.rs `cross_player_tests`:
//! swap-clears-Bottle/Upbyside, Mirror nullify-set/reflect). This file pins the
//! one cross-player interaction that lives purely in bt-core: Lawyers' Delite,
//! which keys off the opponent's line clears.

use bt_core::game::GameEvent;
use bt_core::weapons::WeaponToken;
use bt_core::Game;

fn lock_a_piece(g: &mut Game) {
    g.begin_drop();
    for _ in 0..600 {
        g.tick(16);
        if g.is_game_over() {
            return;
        }
        if g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
            return;
        }
    }
    panic!("piece never locked");
}

fn any_filled(g: &Game) -> bool {
    let b = g.board();
    (0..b.height)
        .flat_map(|y| (0..b.width).map(move |x| (x, y)))
        .any(|(x, y)| b.get(x, y).is_some())
}

/// Lawyers' Delite: while active, every line the OPPONENT clears raises the
/// victim's board by one garbage row. Without it, opponent clears do nothing to
/// the victim's board. (`Game::receive_op_score`, BTGame `BT_LAWYER`.)
#[test]
fn lawyers_delite_raises_board_on_opponent_clears() {
    // Control: no Lawyers — the opponent clearing 5 lines leaves us untouched.
    let mut g = Game::new(1);
    g.receive_op_score(0, 5, 0);
    assert!(!any_filled(&g), "without Lawyers, opponent clears add no garbage");

    // Armed: Lawyers active, opponent clears 3 lines -> 3 garbage rows rise up.
    let mut g = Game::new(1);
    g.receive_weapon(WeaponToken::Lawyers);
    lock_a_piece(&mut g); // flush -> Lawyers active (the dropped piece sits at the floor)
    assert!(g.board().active.is_active(WeaponToken::Lawyers));

    g.receive_op_score(0, 3, 0); // opponent cleared 3 lines

    let b = g.board();
    let (w, h) = (b.width, b.height);
    for r in 0..3 {
        let y = h - 1 - r;
        let filled = (0..w).filter(|&x| b.get(x, y).is_some()).count();
        assert_eq!(
            filled,
            (w - 1) as usize,
            "garbage row {y} should be solid but for a single gap"
        );
    }
}

/// Lawyers only fires on the *delta* of opponent lines, not the absolute count:
/// a repeated report with no new lines inserts nothing further.
#[test]
fn lawyers_fires_only_on_new_opponent_lines() {
    let mut g = Game::new(1);
    g.receive_weapon(WeaponToken::Lawyers);
    lock_a_piece(&mut g);

    g.receive_op_score(0, 2, 0); // +2 lines -> 2 garbage rows
    let after_first: usize = {
        let b = g.board();
        (0..b.height)
            .flat_map(|y| (0..b.width).map(move |x| (x, y)))
            .filter(|&(x, y)| b.get(x, y).is_some())
            .count()
    };

    g.receive_op_score(0, 2, 0); // same total -> no new garbage
    let after_repeat: usize = {
        let b = g.board();
        (0..b.height)
            .flat_map(|y| (0..b.width).map(move |x| (x, y)))
            .filter(|&(x, y)| b.get(x, y).is_some())
            .count()
    };

    assert_eq!(after_first, after_repeat, "a repeated op-line total inserts nothing new");
}
