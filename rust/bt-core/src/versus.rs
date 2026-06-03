//! Two-player match wiring — the cross-player weapon relay plus an authoritative
//! head-to-head match engine.
//!
//! The original BattleTris ran each player's board on their own client and
//! exchanged deltas peer-to-peer (`BTCommManager`). This module hosts BOTH
//! boards in one place so a single authority can tick them in lockstep and
//! resolve the cross-player weapons (Mirror, Swap, Susan, the funds taxes)
//! deterministically. It is consumed two ways:
//!   * [`bt_ai::VsComputer`] (player vs Ernie) — reuses [`deliver_weapon`].
//!   * the server's authoritative online match — owns a [`Versus`] and feeds it
//!     each client's inputs, then ships authoritative snapshots back.
//!
//! Keeping it in `bt-core` (dependency-free) means the netcode never pulls in
//! the AI crate just to relay a weapon.

use crate::game::{Game, GameEvent};
use crate::weapons::WeaponToken;

/// Which side of a head-to-head match. Generic A/B (the player-vs-AI wrapper
/// keeps its own Player/Ai naming and maps onto this only when it needs to).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    A,
    B,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::A => Side::B,
            Side::B => Side::A,
        }
    }
}

/// Weapons a Mirror simply nullifies (fizzles) rather than backfiring, per the
/// original `BTWeaponManager.C:204-216` switch and the Mirror description.
/// Includes Mirror itself (so a curse can't ping-pong) and the spies (D6).
pub fn mirror_nullifies(token: WeaponToken) -> bool {
    use WeaponToken::*;
    matches!(
        token,
        Swap | Mondale | Keating | Ames | Ace | Condor | NiceDay | Susan | Mirror
    )
}

/// Route a weapon launched by `attacker` at `victim`, honoring the OFFENSIVE
/// Mirror (faithful to `BTWeaponManager.C:204-219`).
///
/// Launching Mirror is a normal attack that curses the opponent. While a player
/// is mirror-cursed, every weapon THEY launch is caught by their own curse: the
/// nullify-9 ([`mirror_nullifies`]) fizzle, everything else backfires onto the
/// cursed launcher. An un-cursed launch (Mirror included) hits the opponent.
///
/// Swap/Susan act on both boards at once; every other weapon is queued on its
/// target and lands at that target's next lock (the port's `weapq_` model).
pub fn deliver_weapon(attacker: &mut Game, victim: &mut Game, token: WeaponToken) {
    if attacker.weapon_active(WeaponToken::Mirror) {
        if mirror_nullifies(token) {
            return; // fizzles against the launcher's own mirror curse
        }
        // Backfires onto the cursed launcher (a local BT_WPN_ON in the original;
        // queued to the launcher's next lock here, per the port's weapq model).
        apply_weapon(attacker, victim, token, Recipient::Attacker);
        return;
    }
    apply_weapon(attacker, victim, token, Recipient::Victim);
}

/// Who a (non-Swap/Susan) weapon's effect lands on once Mirror is resolved.
#[derive(Clone, Copy)]
enum Recipient {
    Attacker,
    Victim,
}

fn apply_weapon(attacker: &mut Game, victim: &mut Game, token: WeaponToken, to: Recipient) {
    match token {
        // Swap/Susan are symmetric exchanges (never reach here while cursed —
        // both are on the nullify list — so `to` is always Victim for them).
        WeaponToken::Swap => attacker.swap_board_with(victim),
        WeaponToken::Susan => attacker.swap_arsenal_with(victim),
        _ => match to {
            Recipient::Attacker => attacker.receive_weapon(token),
            Recipient::Victim => victim.receive_weapon(token),
        },
    }
}

/// An authoritative head-to-head match: owns both boards and ticks them in
/// lockstep, resolving the cross-player relay each frame. The host (the server)
/// drives each side's inputs via [`Versus::game_mut`] and reads authoritative
/// state via [`Versus::game`]; it never has to reimplement the weapon relay.
#[derive(Clone, Debug)]
pub struct Versus {
    a: Game,
    b: Game,
    /// 0 = ongoing, 1 = A won (B topped out), 2 = B won (A topped out).
    result: i32,
}

impl Versus {
    /// New match. The two sides get distinct seeds so their piece streams differ
    /// (mirrors the player/AI split in [`bt_ai::VsComputer`]).
    pub fn new(seed_a: u64, seed_b: u64) -> Versus {
        Versus {
            a: Game::new(seed_a),
            b: Game::new(seed_b),
            result: 0,
        }
    }

    pub fn game(&self, side: Side) -> &Game {
        match side {
            Side::A => &self.a,
            Side::B => &self.b,
        }
    }

    /// Mutable side access — the host drives a player's input through this
    /// (`move_left`, `rotate`, `launch_weapon`, `buy_weapon`, `leave_bazaar`, …).
    pub fn game_mut(&mut self, side: Side) -> &mut Game {
        match side {
            Side::A => &mut self.a,
            Side::B => &mut self.b,
        }
    }

    /// 0 = ongoing, 1 = A won, 2 = B won.
    pub fn result(&self) -> i32 {
        self.result
    }

    pub fn is_over(&self) -> bool {
        self.result != 0
    }

    /// Advance the match by `dt_ms`. While EITHER side is shopping the whole
    /// match freezes (the synchronized bazaar barrier — `BTGame` pauses all
    /// timeouts until both players leave); each side clears its own bazaar with a
    /// `leave_bazaar` input, and play resumes only once both have.
    pub fn tick(&mut self, dt_ms: i32) {
        if self.result != 0 {
            return;
        }
        if self.a.is_in_bazaar() || self.b.is_in_bazaar() {
            // Frozen: still relay the events the triggering lock queued
            // (EnterBazaar / Scored) so both mirrors stay in sync, but tick
            // neither game.
            self.relay();
            return;
        }
        self.a.tick(dt_ms);
        self.b.tick(dt_ms);
        self.relay();
    }

    /// Wire weapons / scores / funds between the two boards and latch the result.
    fn relay(&mut self) {
        for e in self.a.take_events() {
            match e {
                GameEvent::WeaponLaunched(t) => deliver_weapon(&mut self.a, &mut self.b, t),
                GameEvent::Scored { score, lines, funds } => {
                    self.b.receive_op_score(score, lines, funds)
                }
                GameEvent::FundsStolen(amount) => self.b.add_funds(amount),
                GameEvent::GameOver => self.result = 2, // A topped out → B wins
                _ => {}
            }
        }
        for e in self.b.take_events() {
            match e {
                GameEvent::WeaponLaunched(t) => deliver_weapon(&mut self.b, &mut self.a, t),
                GameEvent::Scored { score, lines, funds } => {
                    self.a.receive_op_score(score, lines, funds)
                }
                GameEvent::FundsStolen(amount) => self.a.add_funds(amount),
                GameEvent::GameOver => self.result = 1, // B topped out → A wins
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;

    fn cell_count(g: &Game) -> usize {
        let b = g.board();
        (0..b.height)
            .flat_map(|y| (0..b.width).map(move |x| (x, y)))
            .filter(|&(x, y)| b.get(x, y).is_some())
            .count()
    }

    fn lock(g: &mut Game) {
        g.begin_drop();
        for _ in 0..400 {
            g.tick(16);
            if g.is_game_over()
                || g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. }))
            {
                return;
            }
        }
    }

    #[test]
    fn mirror_nullify_set_is_exactly_the_originals_nine() {
        use WeaponToken::*;
        for t in [Swap, Mondale, Keating, Ames, Ace, Condor, NiceDay, Susan, Mirror] {
            assert!(mirror_nullifies(t), "{t:?} should be nullified");
        }
        for t in [RiseUp, Speedy, Bottle, Force, Gimp, FlipOut, Hatter] {
            assert!(!mirror_nullifies(t), "{t:?} should backfire, not nullify");
        }
    }

    #[test]
    fn deliver_weapon_hits_the_opponent_when_uncursed() {
        let mut atk = Game::new(1);
        let mut vic = Game::new(2);
        deliver_weapon(&mut atk, &mut vic, WeaponToken::RiseUp);
        lock(&mut vic);
        assert!(cell_count(&vic) >= 9, "RiseUp landed on the victim");
        assert_eq!(cell_count(&atk), 0, "attacker untouched");
    }

    #[test]
    fn deliver_weapon_backfires_when_the_launcher_is_cursed() {
        let mut atk = Game::new(1);
        let mut vic = Game::new(2);
        // Curse the attacker by delivering Mirror onto them, then lock to arm it.
        deliver_weapon(&mut vic, &mut atk, WeaponToken::Mirror);
        lock(&mut atk);
        assert!(atk.weapon_active(WeaponToken::Mirror));

        deliver_weapon(&mut atk, &mut vic, WeaponToken::RiseUp);
        lock(&mut atk);
        assert!(cell_count(&atk) >= 9, "RiseUp backfired onto the cursed launcher");
        assert_eq!(cell_count(&vic), 0, "victim spared");
    }

    #[test]
    fn cursed_launchers_nullify_weapon_fizzles() {
        let mut atk = Game::new(1);
        let mut vic = Game::new(2);
        deliver_weapon(&mut vic, &mut atk, WeaponToken::Mirror);
        lock(&mut atk);
        atk.board_mut().set(3, 20, Some(Cell::die(5)));
        let (a0, v0) = (cell_count(&atk), cell_count(&vic));

        deliver_weapon(&mut atk, &mut vic, WeaponToken::Swap); // Swap is nullified
        assert_eq!(cell_count(&atk), a0, "cursed Swap fizzled — boards unchanged");
        assert_eq!(cell_count(&vic), v0);
    }

    #[test]
    fn versus_latches_a_winner_when_a_side_tops_out() {
        let mut v = Versus::new(7, 8);
        // Bury side B: fill every column EXCEPT column 0, so no row is ever
        // complete (nothing clears) but the spawn region is blocked. B's piece
        // locks and the next spawn fails -> GameOver. A is still alive, so A wins.
        let (w, h) = (v.game(Side::B).board().width, v.game(Side::B).board().height);
        for y in 0..h {
            for x in 1..w {
                v.game_mut(Side::B).board_mut().set(x, y, Some(Cell::die(1)));
            }
        }
        for _ in 0..500 {
            v.tick(16);
            if v.is_over() {
                break;
            }
        }
        assert_eq!(v.result(), 1, "B topped out, so A wins");
    }

    #[test]
    fn versus_routes_a_weapon_between_the_two_humans() {
        let mut v = Versus::new(1, 2);
        v.game_mut(Side::A).grant_weapon(WeaponToken::RiseUp);
        v.game_mut(Side::A).launch_weapon(0); // A fires RiseUp at B
        // Tick so the relay delivers it, then drive B to a lock to flush it.
        v.tick(16);
        for _ in 0..400 {
            v.game_mut(Side::B).begin_drop();
            v.tick(16);
            if cell_count(v.game(Side::B)) >= 9 {
                break;
            }
        }
        assert!(cell_count(v.game(Side::B)) >= 9, "B received A's RiseUp");
    }
}
