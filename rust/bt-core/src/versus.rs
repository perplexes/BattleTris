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
/// Keating credits the attacker its launch-time funds snapshot here and queues
/// only the victim's seizure; every other weapon is queued on its target and
/// takes effect at that target's next piece lock (the `weapq_` model).
pub fn deliver_weapon(attacker: &mut Game, victim: &mut Game, token: WeaponToken) -> Delivery {
    if attacker.weapon_active(WeaponToken::Mirror) {
        if mirror_nullifies(token) {
            return Delivery::default(); // fizzles against the launcher's own mirror curse
        }
        // Backfires: the effect is queued onto the cursed launcher's own next
        // lock instead of the victim's.
        return apply_weapon(attacker, victim, token, Recipient::Attacker);
    }
    apply_weapon(attacker, victim, token, Recipient::Victim)
}

/// What [`deliver_weapon`] did to the two games, so the relay can emit the matching
/// cross-player event(s) for the affected side. All fields are relative to the
/// `(attacker, victim)` passed in. Swap and Susan record nothing here: they exchange
/// data the client cannot reconstruct from a token, so they ride a keyframe instead.
#[derive(Clone, Copy, Default)]
pub struct Delivery {
    /// A weapon queued onto the VICTIM's next lock (the normal case).
    pub queued_on_victim: Option<WeaponToken>,
    /// A weapon queued onto the ATTACKER's next lock (a Mirror backfire).
    pub queued_on_attacker: Option<WeaponToken>,
    /// Funds credited to the ATTACKER (Keating's launch-snapshot credit).
    pub attacker_credit: Option<i64>,
    /// The delivery exchanged data a client cannot reconstruct from a token (a Swap
    /// board or a Susan arsenal), so it needs a keyframe rather than an event.
    pub needs_keyframe: bool,
}

/// A cross-player effect the relay applied to one side's game, captured so the host
/// can forward it to that side's client to apply to its own local sim. Mirrors the
/// apply-side `bt_replay::Input` variants without bt-core depending on bt-replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayEvent {
    /// A weapon queued onto this side's next lock (`Game::receive_weapon`).
    ReceiveWeapon(WeaponToken),
    /// This side's opponent-score mirror updated (`Game::receive_op_score`); funds are
    /// redacted (a client never learns the opponent's funds except through a spy).
    ReceiveOpScore { score: i64, lines: i64 },
    /// Funds credited to this side (a Mondale tax or a Keating launch credit).
    AddFunds(i64),
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
fn apply_weapon(
    attacker: &mut Game,
    victim: &mut Game,
    token: WeaponToken,
    to: Recipient,
) -> Delivery {
    match token {
        // Swap/Susan exchange between the two players, so they ignore `to`.
        // (They never arrive here while the attacker is cursed, as both are on the
        // nullify list, so a backfired exchange can't occur.)
        //
        // Swap captures each board at LAUNCH and installs the other's at that
        // side's next piece lock (board_buf_, BTCommManager.C:448-449,584-588), so
        // the exchange lands at a clean boundary, never under a falling piece.
        // Susan trades arsenals immediately, which the original also does on
        // arsenal-packet receipt (not at a lock; BTWeaponManager.C:104-110).
        //
        // Neither records a RelayEvent: the client can't reconstruct the exchanged
        // board / arsenal from a token, so they ride a keyframe (Delivery::default).
        WeaponToken::Swap => {
            let a_cells = attacker.export_board();
            let v_cells = victim.export_board();
            victim.queue_board_swap(a_cells);
            attacker.queue_board_swap(v_cells);
            Delivery { needs_keyframe: true, ..Delivery::default() }
        }
        WeaponToken::Susan => {
            attacker.swap_arsenal_with(victim);
            Delivery { needs_keyframe: true, ..Delivery::default() }
        }
        // Keating credits the attacker the victim's funds as of LAUNCH (the
        // attacker's cached `op_funds` in the original, BTScoreManager.C:110-111,
        // 151-153), while the victim is zeroed only when the weapon activates at
        // its next lock (Game::apply_weapon_on, BTScoreManager.C:121-123). The
        // launch snapshot and the activation balance can differ, so the credited
        // amount need not equal the seized amount. Keating is mirror-nullified, so
        // `to` is always Victim here.
        WeaponToken::Keating => {
            let credit = victim.score().funds;
            attacker.add_funds(credit);
            victim.receive_weapon(token);
            Delivery {
                queued_on_victim: Some(token),
                attacker_credit: Some(credit),
                ..Delivery::default()
            }
        }
        _ => match to {
            Recipient::Attacker => {
                attacker.receive_weapon(token);
                Delivery { queued_on_attacker: Some(token), ..Delivery::default() }
            }
            Recipient::Victim => {
                victim.receive_weapon(token);
                Delivery { queued_on_victim: Some(token), ..Delivery::default() }
            }
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
    /// Set when this tick produced something the client can't apply from an event and
    /// so needs a full keyframe: a bazaar entry, or a Swap / Susan exchange (which move
    /// the opponent's board / arsenal, data a token cannot carry). Ordinary weapons,
    /// funds taxes, and op-score updates ride the event channel ([`Versus::take_outbox`])
    /// instead. The host reads this via [`Versus::take_dirty`].
    dirty: bool,
    /// Spies launched THIS tick by each side (A = [0], B = [1]), for the host to
    /// pick up (the spy reveal is a host-level concern handled outside the board). A Vec so
    /// several launches drained in one server tick all accumulate.
    spy_launch: [Vec<WeaponToken>; 2],
    /// Cross-player effects the relay applied to each side's game this tick (A = [0],
    /// B = [1]), for the host to forward to that side's client so its local sim applies
    /// the same effect (the model-B event channel). Drained with [`Versus::take_outbox`].
    outbox: [Vec<RelayEvent>; 2],
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
            outbox: [Vec::new(), Vec::new()],
        }
    }

    /// Take (and clear) the cross-player events the relay applied to `side`'s game on
    /// the last tick, in order. The host forwards them to that side's client so its
    /// local sim applies the same effects without waiting for a keyframe.
    pub fn take_outbox(&mut self, side: Side) -> Vec<RelayEvent> {
        std::mem::take(&mut self.outbox[match side {
            Side::A => 0,
            Side::B => 1,
        }])
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
    ///
    /// Besides applying each cross-player effect, this records it in the per-side
    /// `outbox` (the host forwards those to the client whose local sim must apply the
    /// same effect). The two halves are symmetric: `A`'s events act on `B` (or backfire
    /// onto `A`), and the matching events are recorded for whichever side received
    /// them. Indices: `A = 0`, `B = 1`.
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
                        let d = deliver_weapon(&mut self.a, &mut self.b, t);
                        self.record_delivery(d, 0, 1);
                        // Only a Swap / Susan exchange needs a keyframe; ordinary
                        // weapons reach the client through the recorded event.
                        if d.needs_keyframe {
                            self.dirty = true;
                        }
                    }
                }
                GameEvent::Scored { score, lines, funds } => {
                    self.b.receive_op_score(score, lines, funds);
                    self.outbox[1].push(RelayEvent::ReceiveOpScore { score, lines });
                }
                GameEvent::FundsStolen(amount) => {
                    self.b.add_funds(amount);
                    self.outbox[1].push(RelayEvent::AddFunds(amount));
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
                        let d = deliver_weapon(&mut self.b, &mut self.a, t);
                        self.record_delivery(d, 1, 0);
                        if d.needs_keyframe {
                            self.dirty = true;
                        }
                    }
                }
                GameEvent::Scored { score, lines, funds } => {
                    self.a.receive_op_score(score, lines, funds);
                    self.outbox[0].push(RelayEvent::ReceiveOpScore { score, lines });
                }
                GameEvent::FundsStolen(amount) => {
                    self.a.add_funds(amount);
                    self.outbox[0].push(RelayEvent::AddFunds(amount));
                }
                GameEvent::EnterBazaar => self.dirty = true,
                GameEvent::GameOver if self.result == 0 => self.result = 1, // B topped out → A wins
                _ => {}
            }
        }
    }

    /// Record a [`deliver_weapon`] outcome into the outbox, given the attacker and
    /// victim side indices for this delivery.
    fn record_delivery(&mut self, d: Delivery, attacker_idx: usize, victim_idx: usize) {
        if let Some(t) = d.queued_on_victim {
            self.outbox[victim_idx].push(RelayEvent::ReceiveWeapon(t));
        }
        if let Some(t) = d.queued_on_attacker {
            self.outbox[attacker_idx].push(RelayEvent::ReceiveWeapon(t));
        }
        if let Some(amount) = d.attacker_credit {
            self.outbox[attacker_idx].push(RelayEvent::AddFunds(amount));
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
    fn swap_installs_the_opponents_board_at_the_next_lock_not_immediately() {
        // The original buffers the swapped board (board_buf_) and installs it in
        // flushWeapons at the victim's next lock (BTCommManager.C:448-449,584-588),
        // so the exchange never mutates a board under a falling piece.
        let mut a = Game::new(1);
        let mut b = Game::new(2);
        a.board_mut().set(0, 22, Some(Cell::die(3)));
        b.board_mut().set(9, 22, Some(Cell::die(5)));
        let a_board = a.export_board();
        let b_board = b.export_board();

        // A launches Swap at B: the relay queues the exchange on both sides.
        deliver_weapon(&mut a, &mut b, WeaponToken::Swap);
        assert_eq!(b.export_board(), b_board, "deferred: B's board is unchanged until B locks");
        assert_eq!(a.export_board(), a_board, "deferred: A's board is unchanged until A locks");

        // B locks first and installs A's launch-time board; A is still pending.
        lock(&mut b);
        assert_eq!(b.export_board(), a_board, "B installed A's board at B's lock");
        assert_eq!(a.export_board(), a_board, "A still holds its own board until A locks");

        // A locks and installs B's launch-time board.
        lock(&mut a);
        assert_eq!(a.export_board(), b_board, "A installed B's board at A's lock");
    }

    #[test]
    fn keating_credits_launch_snapshot_not_activation_balance() {
        // The original credits the attacker its cached `op_funds` snapshotted at
        // LAUNCH (BTScoreManager.C:110-111,151-153) and zeroes the victim only
        // when the weapon activates at the victim's next lock (:121-123). When the
        // victim's balance grows between those two moments, the credited amount is
        // less than the seized amount and the difference vanishes.
        let mut atk = Game::new(1);
        let mut vic = Game::new(2);
        let atk0 = atk.score().funds;
        vic.add_funds(100);
        vic.take_events();
        let launch_funds = vic.score().funds;

        deliver_weapon(&mut atk, &mut vic, WeaponToken::Keating);
        assert_eq!(atk.score().funds, atk0 + launch_funds, "attacker credited the launch snapshot");
        assert_eq!(vic.score().funds, launch_funds, "victim not yet zeroed (activates at next lock)");

        // The victim earns more before the queued Keating activates.
        vic.add_funds(50);
        vic.take_events();
        let activation_funds = vic.score().funds;
        assert!(activation_funds > launch_funds, "victim's balance grew after launch");

        lock(&mut vic);
        assert_eq!(vic.score().funds, 0, "victim loses its full activation balance");
        assert_eq!(
            atk.score().funds,
            atk0 + launch_funds,
            "attacker keeps only the launch snapshot, below the {activation_funds} seized",
        );
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
    fn an_ordinary_weapon_rides_the_event_channel_not_a_keyframe() {
        // Model B: a token-only weapon (here RiseUp, a board raiser) reaches the client
        // as a recorded event, so it must NOT mark the match dirty. Marking dirty would
        // force an unnecessary full keyframe, which is exactly the periodic reconcile
        // this stage retires. The delivery instead shows up in the victim's outbox.
        let mut v = Versus::new(1, 2);
        assert!(!v.take_dirty(), "clean at the start");
        v.game_mut(Side::A).grant_weapon(WeaponToken::RiseUp);
        v.game_mut(Side::A).launch_weapon(0);
        v.tick(16); // relay delivers the weapon to B
        assert!(!v.take_dirty(), "an ordinary weapon rides the event channel, no keyframe");
        let events = v.take_outbox(Side::B);
        assert!(
            events.iter().any(|e| matches!(e, RelayEvent::ReceiveWeapon(_))),
            "the delivery is recorded as an event the host forwards to B"
        );
    }

    #[test]
    fn a_swap_marks_the_match_dirty() {
        // Swap (and Susan) move data a token cannot carry (the opponent's whole board /
        // arsenal), so they still demand a keyframe and must set dirty. This is the
        // narrow set the model-B keyframe is kept for.
        let mut v = Versus::new(1, 2);
        assert!(!v.take_dirty(), "clean at the start");
        v.game_mut(Side::A).grant_weapon(WeaponToken::Swap);
        v.game_mut(Side::A).launch_weapon(0);
        v.tick(16); // relay queues the board swap on both sides
        assert!(v.take_dirty(), "a Swap exchange needs a keyframe, so it marks dirty");
        v.tick(16);
        assert!(!v.take_dirty(), "and it clears after being taken");
    }
}
