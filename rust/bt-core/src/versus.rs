//! Two-player match wiring: the cross-player weapon relay and an authoritative
//! head-to-head match engine.
//!
//! Cross-player weapons (Mirror, Swap, Susan, the funds taxes) touch BOTH
//! players, which a single [`Game`] cannot resolve on its own. So this module
//! holds both boards in one place and ticks them in lockstep, giving one
//! authority that resolves the relay deterministically. It is consumed two ways:
//!   * `bt_ai::VsComputer` (player vs Ernie), which reuses [`deliver_weapon`].
//!   * the server's authoritative online match, which owns a [`Versus`], feeds each
//!     client's inputs in, and ships authoritative snapshots back.
//!
//! It lives in `bt-core` (which stays dependency-free) so the netcode can relay
//! a weapon without pulling in the AI crate.

use crate::game::{Game, GameEvent};
use crate::weapons::WeaponToken;

/// Which side of a head-to-head match. Deliberately anonymous A/B rather than
/// player/opponent; the relay is symmetric, and the player-vs-AI wrapper keeps
/// its own Player/Ai naming, mapping onto these only at the boundary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    A,
    B,
}

impl Side {
    /// The opposing side, for the relay's "deliver to the other player" routing.
    pub fn other(self) -> Side {
        match self {
            Side::A => Side::B,
            Side::B => Side::A,
        }
    }
}

/// Whether `token` is a spy (Ames/Ace/Condor). Spies are information weapons:
/// they reveal the opponent's board TO the launcher instead of hitting the
/// opponent. [`Versus`]'s relay uses this to peel spies out of the weapon stream
/// and record them as a host-side reveal rather than [`deliver_weapon`]-ing them
/// (a host that calls `deliver_weapon` directly is responsible for the same
/// filtering).
pub fn is_spy(token: WeaponToken) -> bool {
    matches!(token, WeaponToken::Ames | WeaponToken::Ace | WeaponToken::Condor)
}

/// Whether a Mirror makes `token` fizzle harmlessly rather than backfire onto a
/// cursed launcher (`BTWeaponManager.C:204-216`).
///
/// A weapon fizzles when reflecting it onto its own launcher is meaningless:
/// Swap/Susan would exchange a player's board or arsenal with itself;
/// Keating/Mondale skim funds you would be taking from yourself; Have a Nice Day
/// gifting yourself a smiley is harmless; the spies reveal you to yourself; and
/// Mirror reflecting Mirror would loop. Every weapon NOT on this list backfires
/// with real effect (e.g. Reagan negates the cursed launcher's own funds).
pub fn mirror_nullifies(token: WeaponToken) -> bool {
    use WeaponToken::*;
    matches!(
        token,
        Swap | Mondale | Keating | Ames | Ace | Condor | NiceDay | Susan | Mirror
    )
}

/// Route a weapon launched by `attacker` at `victim`, honoring the offensive
/// Mirror (`BTWeaponManager.C:204-219`).
///
/// Launching Mirror is itself a normal attack that curses the opponent. The
/// twist is what happens while a player IS mirror-cursed: their own curse
/// catches every weapon they launch: [`mirror_nullifies`] ones fizzle, all
/// others backfire onto the cursed launcher. An un-cursed launch (Mirror
/// included) hits the opponent as normal.
///
/// Swap and Susan act on both boards at once and are applied here immediately;
/// every other weapon is queued on its target and takes effect at that target's
/// next piece lock (the `weapq_` model).
pub fn deliver_weapon(attacker: &mut Game, victim: &mut Game, token: WeaponToken) {
    if attacker.weapon_active(WeaponToken::Mirror) {
        if mirror_nullifies(token) {
            return; // fizzles against the launcher's own mirror curse
        }
        // Backfires: the effect is queued onto the cursed launcher's own next
        // lock instead of the victim's.
        apply_weapon(attacker, victim, token, Recipient::Attacker);
        return;
    }
    apply_weapon(attacker, victim, token, Recipient::Victim);
}

/// Which player a (non-Swap/Susan) weapon's effect lands on once Mirror has been
/// resolved: the victim normally, or back onto the attacker when a curse
/// backfired it.
#[derive(Clone, Copy)]
enum Recipient {
    /// The launcher (a backfired weapon).
    Attacker,
    /// The intended target.
    Victim,
}

/// Apply `token` after Mirror resolution: the symmetric exchanges act on both
/// boards at once; everything else is queued onto whichever player `to` names.
fn apply_weapon(attacker: &mut Game, victim: &mut Game, token: WeaponToken, to: Recipient) {
    match token {
        // Swap/Susan exchange between the two players, so they ignore `to`.
        // (They never arrive here while the attacker is cursed, as both are on the
        // nullify list, so a backfired exchange can't occur.)
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
    /// 0 = ongoing, 1 = A won (B topped out), 2 = B won (A topped out). Latched
    /// once set, so the first side to top out decides the match.
    result: i32,
    /// Set whenever this tick produced something a CLIENT can't predict from its
    /// own inputs: a cross-player weapon delivery, a funds tax/steal, or a bazaar
    /// entry. The host reads it via [`Versus::take_dirty`] to push a prompt
    /// reconciliation keyframe instead of waiting for the periodic heartbeat.
    dirty: bool,
    /// Spies launched THIS tick by each side (A = [0], B = [1]), for the host to
    /// pick up (the spy reveal is a host-level concern handled outside the board). A Vec so
    /// several launches drained in one server tick all accumulate.
    spy_launch: [Vec<WeaponToken>; 2],
}

impl Versus {
    /// New match. The two sides get distinct seeds so their piece streams differ
    /// (mirrors the player/AI split in `bt_ai::VsComputer`).
    pub fn new(seed_a: u64, seed_b: u64) -> Versus {
        Versus {
            a: Game::new(seed_a),
            b: Game::new(seed_b),
            result: 0,
            dirty: false,
            spy_launch: [Vec::new(), Vec::new()],
        }
    }

    /// Take (and clear) the "client can't predict this" flag, set by the last
    /// tick's cross-player relay (weapon, funds, bazaar entry). The server uses
    /// it to send a prompt keyframe rather than wait for the periodic one.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// Take (and clear) the spies `side` launched on the last tick, in order (the
    /// host tracks each spy's line-based duration and reveals the opponent board).
    pub fn take_spy_launches(&mut self, side: Side) -> Vec<WeaponToken> {
        std::mem::take(&mut self.spy_launch[match side {
            Side::A => 0,
            Side::B => 1,
        }])
    }

    /// Read-only access to one side's game (for rendering and authoritative
    /// snapshots).
    pub fn game(&self, side: Side) -> &Game {
        match side {
            Side::A => &self.a,
            Side::B => &self.b,
        }
    }

    /// Mutable side access. The host drives a player's input through this
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

    /// Whether a side has topped out and the match is decided.
    pub fn is_over(&self) -> bool {
        self.result != 0
    }

    /// Advance the match by `dt_ms`. While EITHER side is shopping the whole
    /// match freezes (the synchronized bazaar barrier; `BTGame` pauses all
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
                GameEvent::WeaponLaunched(t) => {
                    if is_spy(t) {
                        // A spy reveals the opponent to the LAUNCHER (a host
                        // concern), never delivered to the opponent, unless the
                        // launcher is mirror-cursed, in which case it fizzles
                        // like any reflected info weapon.
                        if !self.a.weapon_active(WeaponToken::Mirror) {
                            self.spy_launch[0].push(t);
                        }
                    } else {
                        deliver_weapon(&mut self.a, &mut self.b, t);
                    }
                    self.dirty = true;
                }
                GameEvent::Scored { score, lines, funds } => {
                    self.b.receive_op_score(score, lines, funds)
                }
                GameEvent::FundsStolen(amount) => {
                    self.b.add_funds(amount);
                    self.dirty = true;
                }
                GameEvent::EnterBazaar => self.dirty = true,
                // A topped out → B wins. Latch: a simultaneous double-KO keeps the
                // first result (whoever's GameOver this relay pass saw first),
                // rather than letting the second event overwrite it.
                GameEvent::GameOver if self.result == 0 => self.result = 2,
                _ => {}
            }
        }
        for e in self.b.take_events() {
            match e {
                GameEvent::WeaponLaunched(t) => {
                    if is_spy(t) {
                        if !self.b.weapon_active(WeaponToken::Mirror) {
                            self.spy_launch[1].push(t);
                        }
                    } else {
                        deliver_weapon(&mut self.b, &mut self.a, t);
                    }
                    self.dirty = true;
                }
                GameEvent::Scored { score, lines, funds } => {
                    self.a.receive_op_score(score, lines, funds)
                }
                GameEvent::FundsStolen(amount) => {
                    self.a.add_funds(amount);
                    self.dirty = true;
                }
                GameEvent::EnterBazaar => self.dirty = true,
                GameEvent::GameOver if self.result == 0 => self.result = 1, // B topped out → A wins
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
        assert_eq!(cell_count(&atk), a0, "cursed Swap fizzled; boards unchanged");
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

    #[test]
    fn a_spy_is_recorded_for_the_launcher_and_not_delivered_to_the_opponent() {
        let mut v = Versus::new(1, 2);
        v.game_mut(Side::A).grant_weapon(WeaponToken::Ames);
        v.game_mut(Side::A).launch_weapon(0);
        v.tick(16); // the relay records the spy launch for the host
        assert_eq!(v.take_spy_launches(Side::A), vec![WeaponToken::Ames], "launcher's spy recorded");
        assert!(v.take_spy_launches(Side::B).is_empty());
        // The opponent must NOT receive the spy as a weapon (it's info-only).
        lock(v.game_mut(Side::B));
        assert!(
            !v.game(Side::B).weapon_active(WeaponToken::Ames),
            "the opponent is unaffected by being spied on"
        );
    }

    #[test]
    fn a_cross_player_weapon_marks_the_match_dirty() {
        let mut v = Versus::new(1, 2);
        assert!(!v.take_dirty(), "clean at the start");
        v.game_mut(Side::A).grant_weapon(WeaponToken::RiseUp);
        v.game_mut(Side::A).launch_weapon(0);
        v.tick(16); // relay delivers the weapon to B
        assert!(v.take_dirty(), "a delivered weapon marks the match dirty");
        v.tick(16);
        assert!(!v.take_dirty(), "and it clears after being taken");
    }
}
