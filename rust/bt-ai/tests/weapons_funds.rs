//! Per-weapon oracle — funds effects (Keating, Reagan, Mondale).
//!
//! These need a victim with money in the bank, so we let Ernie actually play
//! until he's earned funds from line clears, then deliver the weapon and assert
//! the exact effect on `score.funds`. Ernie is driven event-style (one move per
//! settled piece, like `VsComputer`) so the placements are faithful.

use bt_ai::Computer;
use bt_core::constants::BT_MONDALE_RATE;
use bt_core::game::GameEvent;
use bt_core::weapons::WeaponToken;
use bt_core::Game;

/// Faithful one-move-per-piece driver for a solo Game (mirrors VsComputer's
/// `ai_committed` gate; leaves the bazaar immediately so a solo run keeps going).
struct Driver {
    ernie: Computer,
    committed: bool,
}

impl Driver {
    fn new() -> Self {
        Driver { ernie: Computer::new(), committed: false }
    }

    /// Advance one frame; returns the events produced this frame.
    fn step(&mut self, g: &mut Game) -> Vec<GameEvent> {
        if g.is_in_bazaar() {
            g.leave_bazaar(); // solo: nobody else to wait on
        }
        if !self.committed && g.current_piece().is_some() {
            self.ernie.take_turn(g);
            self.committed = true;
        }
        g.tick(16);
        let evs = g.take_events();
        if evs.iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
            self.committed = false;
        }
        evs
    }

    fn run_until(&mut self, g: &mut Game, mut stop: impl FnMut(&Game) -> bool, max: usize) -> bool {
        for _ in 0..max {
            if stop(g) {
                return true;
            }
            if g.is_game_over() {
                return false;
            }
            self.step(g);
        }
        false
    }
}

/// Keating Five: the victim's funds are all taken away (set to 0).
#[test]
fn keating_zeroes_funds() {
    let mut g = Game::new(2024);
    let mut d = Driver::new();

    assert!(
        d.run_until(&mut g, |g| g.score().funds > 0, 6000),
        "Ernie should earn some funds to steal"
    );

    g.receive_weapon(WeaponToken::Keating);
    assert!(
        d.run_until(&mut g, |g| g.board().active.is_active(WeaponToken::Keating), 3000),
        "Keating should flush in at the next lock"
    );

    assert_eq!(g.score().funds, 0, "Keating zeroes the victim's funds");
}

/// Reagan Era: the victim's funds are multiplied by -1.
#[test]
fn reagan_negates_funds() {
    let mut g = Game::new(2024);
    let mut d = Driver::new();

    assert!(d.run_until(&mut g, |g| g.score().funds > 0, 6000), "Ernie should earn funds");
    let before = g.score().funds;
    assert!(before > 0);

    g.receive_weapon(WeaponToken::Reagan);
    assert!(
        d.run_until(&mut g, |g| g.board().active.is_active(WeaponToken::Reagan), 3000),
        "Reagan should flush at the next lock"
    );

    assert!(
        g.score().funds < 0,
        "Reagan flips funds negative (was {before}, now {})",
        g.score().funds
    );
}

/// Mondale '96: while active, the victim keeps only 70% of funds earned per
/// clear (a 30% tax, BT_MONDALE_RATE). The Locked event still reports the gross
/// value; the credited delta is the net.
#[test]
fn mondale_taxes_line_funds_by_thirty_percent() {
    let mut g = Game::new(2024);
    let mut d = Driver::new();

    g.receive_weapon(WeaponToken::Mondale);
    assert!(
        d.run_until(&mut g, |g| g.board().active.is_active(WeaponToken::Mondale), 3000),
        "Mondale should flush in"
    );

    // Find the first lock that clears value, and check the credited delta.
    let mut verified = false;
    for _ in 0..6000 {
        if g.is_game_over() {
            break;
        }
        let before = g.score().funds;
        let evs = d.step(&mut g);
        let gross: i64 = evs
            .iter()
            .filter_map(|e| match e {
                GameEvent::Locked { funds, .. } if *funds > 0 => Some(*funds as i64),
                _ => None,
            })
            .sum();
        if gross > 0 {
            let credited = g.score().funds - before;
            let expected = (gross as f64 * (1.0 - BT_MONDALE_RATE)) as i64;
            assert_eq!(
                credited, expected,
                "Mondale: gross {gross} should credit {expected} (70%), got {credited}"
            );
            verified = true;
            break;
        }
    }
    assert!(verified, "expected at least one taxed line clear while Mondale was active");
}
