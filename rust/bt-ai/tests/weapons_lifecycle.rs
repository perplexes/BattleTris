//! Weapons layer 2 — duration lifecycle.
//!
//! Timed weapons are measured in *lines*: a weapon with duration D stays active
//! until the victim has cleared D lines, then expires and reverts. The exact
//! invariant (which the engine's `remaining[] -= lines` countdown guarantees):
//!
//!     active  ⇔  (lines cleared since it landed) < duration
//!
//! holds at every observation, even across multi-line clears. We drive Ernie to
//! generate real line clears and assert that invariant straight through expiry,
//! then confirm the effect actually reverted.

use bt_ai::Computer;
use bt_core::game::GameEvent;
use bt_core::weapons::{weapon_table, WeaponToken};
use bt_core::Game;

struct Driver {
    ernie: Computer,
    committed: bool,
}

impl Driver {
    fn new() -> Self {
        Driver { ernie: Computer::new(), committed: false }
    }
    fn step(&mut self, g: &mut Game) {
        if g.is_in_bazaar() {
            g.leave_bazaar();
        }
        if !self.committed && g.current_piece().is_some() {
            self.ernie.take_turn(g);
            self.committed = true;
        }
        g.tick(16);
        if g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
            self.committed = false;
        }
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

/// Deliver `weapon`, flush it at a lock, and return the victim's line count at
/// that moment (the baseline the duration counts from).
fn land_weapon(g: &mut Game, d: &mut Driver, weapon: WeaponToken) -> i64 {
    g.receive_weapon(weapon);
    assert!(
        d.run_until(g, |g| g.board().active.is_active(weapon), 4000),
        "{weapon:?} should flush in at the next lock"
    );
    g.score().lines
}

/// The core lifecycle invariant, checked through expiry for a few timed weapons
/// that don't disrupt Ernie's ability to keep clearing lines.
#[test]
fn timed_weapons_expire_after_their_duration_in_lines() {
    for (seed, weapon) in [
        (2024u64, WeaponToken::Speedy), // 10 lines
        (99, WeaponToken::SoLong),      // 10 lines
        (7, WeaponToken::Carter),       // 20 lines
    ] {
        let duration = weapon_table()[weapon.index()].duration as i64;
        let mut g = Game::new(seed);
        let mut d = Driver::new();
        let base = land_weapon(&mut g, &mut d, weapon);

        let mut saw_active = false;
        let mut saw_expiry = false;
        for _ in 0..60_000 {
            if g.is_game_over() {
                break;
            }
            let cleared = g.score().lines - base;
            let active = g.board().active.is_active(weapon);
            assert_eq!(
                active,
                cleared < duration,
                "{weapon:?}: active={active} after {cleared}/{duration} lines (invariant violated)"
            );
            saw_active |= active;
            if !active {
                saw_expiry = true;
                break;
            }
            d.step(&mut g);
        }
        assert!(saw_active, "{weapon:?} should be observed active");
        assert!(saw_expiry, "{weapon:?} should be observed expiring within the budget");
    }
}

/// Revert check: Carter doubles bazaar prices while active and the price snaps
/// back the instant it expires (the active flag drives `bazaar_price`).
#[test]
fn carter_price_doubling_reverts_on_expiry() {
    let mut g = Game::new(7);
    let mut d = Driver::new();
    let probe = WeaponToken::Speedy;
    let base_price = g.bazaar_price(probe);

    let base = land_weapon(&mut g, &mut d, WeaponToken::Carter);
    assert_eq!(g.bazaar_price(probe), base_price * 2, "doubled while Carter is active");

    let duration = weapon_table()[WeaponToken::Carter.index()].duration as i64;
    assert!(
        d.run_until(
            &mut g,
            |g| !g.board().active.is_active(WeaponToken::Carter),
            60_000,
        ),
        "Carter should expire"
    );
    assert!(g.score().lines - base >= duration, "expired only after its duration in lines");
    assert_eq!(g.bazaar_price(probe), base_price, "price snaps back to normal once Carter expires");
}
