//! The single-player "vs computer" match, implementing the original `BattleTris -X` mode.
//!
//! [`VsComputer`] owns the human player's [`Game`] plus Ernie's [`Game`] and
//! relays weapons / scores between them, applies the bazaar barrier, and
//! throttles Ernie's placement to the chosen difficulty. It is plain Rust (no
//! wasm-bindgen), so it can be driven headlessly in tests by advancing
//! [`VsComputer::tick`] over a virtual clock (see `tests/vs_computer.rs`).
//!
//! `bt-wasm`'s `WasmVsComputer` is a thin wrapper that adds the JS-facing event
//! encoding around this engine.

use crate::Computer;
use bt_core::game::GameEvent;
use bt_core::weapons::WeaponToken;
use bt_core::Game;

/// Ernie's difficulty table: the per-move delays (ms) from the original
/// `BTComputer.C` `levels[]` (Comatose through Bionic). The challenge screen's
/// "Ernie slider" picks one of these; the page exposes the same choice.
pub const AI_LEVELS: [i32; 15] = [
    4000, 3000, 2000, 1500, 1250, 1000, 750, 550, 400, 350, 300, 225, 100, 10, 0,
];

/// Which side of the match a weapon came from / is headed to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    Player,
    Ai,
}

impl Side {
    #[cfg(test)]
    fn other(self) -> Side {
        match self {
            Side::Player => Side::Ai,
            Side::Ai => Side::Player,
        }
    }
}

/// A single-tab game vs the computer opponent (Ernie). Owns the player's game
/// plus the AI's game and relays weapons / scores between them internally.
#[derive(Clone, Debug)]
pub struct VsComputer {
    player: Game,
    ai: Game,
    computer: Computer,
    /// Ms between AI placements (the chosen difficulty's `levels[].timeout`).
    place_period: i32,
    place_accum: i32,
    /// True once Ernie has steered the current piece into its drop. `take_turn`
    /// only fires on a fresh piece: it ends with `ai_begin_drop` (a fast-drop
    /// that takes several ticks to land rather than an instant placement), so
    /// without this gate a short `place_period` would re-fire on the still-falling
    /// piece and steer it mid-flight into a self-topping tower. The original
    /// computer was event-driven (one move per settled piece); this reproduces
    /// that. Reset when the AI locks a piece (see [`VsComputer::relay`]).
    ai_committed: bool,
    /// 0 = ongoing, 1 = player won, 2 = player lost.
    result: i32,
    /// Player-side events surfaced for rendering (raw; the wasm layer encodes
    /// them as i32 quads for the Canvas front-end).
    events: Vec<GameEvent>,
}

impl VsComputer {
    /// `level` indexes [`AI_LEVELS`] (0 = Comatose … 14 = Bionic), mirroring the
    /// original's Ernie-difficulty slider; out-of-range clamps to the table.
    pub fn new(seed: u64, level: usize) -> VsComputer {
        let idx = level.min(AI_LEVELS.len() - 1);
        // Ernie's first move is throttled like every other one: the original
        // `BTComputer` schedules it one `delay_` after `BT_START`
        // (BTComputer.C, `addTimeout(delay_, ...)`), it does NOT place at t=0.
        // So the constructor must not place a piece; otherwise even a Comatose
        // (4000ms) Ernie would bank its first piece, and the score that comes
        // with it, before the first tick. `ai_committed` starts false so the
        // first `take_turn` waits out `place_accum >= place_period`.
        VsComputer {
            player: Game::new(seed),
            ai: Game::new(seed ^ 0x9E37_79B9_7F4A_7C15),
            computer: Computer::new(seed),
            place_period: AI_LEVELS[idx],
            place_accum: 0,
            ai_committed: false,
            result: 0,
            events: Vec::new(),
        }
    }

    /// Advance the match by `dt_ms` of virtual time.
    pub fn tick(&mut self, dt_ms: i32) {
        if self.result != 0 {
            return;
        }

        // Bazaar barrier: `BTGame` pauses all timeouts on `BT_START_BAZ` and
        // resumes only once both sides have left (see BattleTris(1) and
        // `BTComputer::checkBazaar`). Both games enter together at every 20th
        // combined line; while the human shops, the whole match is frozen so
        // Ernie can't rack up free real-time turns. Ernie does its one-shot
        // shopping on entry, then waits for the human's DONE (which the page
        // signals via `leave_bazaar`).
        if self.player.is_in_bazaar() || self.ai.is_in_bazaar() {
            if self.ai.is_in_bazaar() {
                // Ernie shops via the commando engine: buy weapons per the
                // whitelist/combo logic and queue their launch orders, then leave.
                self.computer.shop(&mut self.ai);
                self.ai.leave_bazaar();
            }
            // Forward the EnterBazaar / Scored events queued by the triggering
            // lock; neither game ticks, so nothing new is produced.
            self.relay();
            return;
        }

        self.player.tick(dt_ms);
        self.ai_logic(dt_ms);
        self.relay();
    }

    fn ai_logic(&mut self, dt: i32) {
        self.ai.tick(dt);

        // Place the current piece on a throttle so it's watchable (the chosen
        // difficulty's per-move delay). Only steer a fresh piece, one we
        // haven't already committed to a drop (see `ai_committed`).
        self.place_accum += dt;
        if !self.ai_committed && self.place_accum >= self.place_period {
            self.place_accum = 0;
            if !self.ai.is_game_over() && self.ai.current_piece().is_some() {
                // take_turn fires any triggered weapon orders, then places the
                // piece (launches are per-placement, as in BTComputer::run).
                self.computer.take_turn(&mut self.ai);
                self.ai_committed = true;
            }
        }
    }

    /// Wire weapons / scores between the two games and capture player-side
    /// events for rendering. `result` is latched the first time either side
    /// tops out (1 = player won, 2 = player lost).
    fn relay(&mut self) {
        for e in self.player.take_events() {
            match e {
                GameEvent::WeaponLaunched(t) => self.deliver(t, Side::Player),
                GameEvent::Scored { score, lines, funds } => {
                    self.ai.receive_op_score(score, lines, funds)
                }
                // The player (victim) was taxed/robbed; pay the attacker (Ernie).
                GameEvent::FundsStolen(amount) => self.ai.add_funds(amount),
                GameEvent::GameOver => self.result = 2,
                _ => {}
            }
            self.events.push(e);
        }
        for e in self.ai.take_events() {
            match e {
                GameEvent::WeaponLaunched(t) => self.deliver(t, Side::Ai),
                GameEvent::Scored { score, lines, funds } => {
                    self.player.receive_op_score(score, lines, funds)
                }
                // Ernie (victim) was taxed/robbed; pay the attacker (player).
                GameEvent::FundsStolen(amount) => self.player.add_funds(amount),
                // The AI's piece settled: ready a fresh one and restart the
                // per-move delay from this lock.
                GameEvent::Locked { .. } => {
                    self.ai_committed = false;
                    self.place_accum = 0;
                }
                GameEvent::GameOver => self.result = 1,
                _ => {}
            }
        }
    }

    /// Route a launched weapon from `attacker` to its target via the shared
    /// offensive-Mirror relay in [`bt_core::versus::deliver_weapon`], which is
    /// the same logic the server's authoritative human-vs-human match uses so
    /// the two modes stay consistent. (Mirror is offensive: launching it curses
    /// the opponent; a cursed launcher's own weapons backfire or fizzle.)
    fn deliver(&mut self, token: WeaponToken, attacker: Side) {
        match attacker {
            Side::Player => bt_core::deliver_weapon(&mut self.player, &mut self.ai, token),
            Side::Ai => bt_core::deliver_weapon(&mut self.ai, &mut self.player, token),
        }
    }

    #[cfg(test)]
    fn game(&self, side: Side) -> &Game {
        match side {
            Side::Player => &self.player,
            Side::Ai => &self.ai,
        }
    }

    /// 0 = ongoing, 1 = player won, 2 = player lost.
    pub fn result(&self) -> i32 {
        self.result
    }

    /// The human player's game (read-only, for rendering and inspection).
    pub fn player(&self) -> &Game {
        &self.player
    }

    /// The human player's game (mutable; the host drives input through this).
    pub fn player_mut(&mut self) -> &mut Game {
        &mut self.player
    }

    /// Ernie's game (read-only, for the optional spectator view).
    pub fn ai(&self) -> &Game {
        &self.ai
    }

    /// Take the queued player-side events (for the host to render). Cleared.
    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod cross_player_tests {
    //! The cross-player weapons (Swap / Susan / Mirror) live in the relay, so
    //! these drive `deliver` directly with in-module access to both games.
    use super::*;
    use bt_core::cell::Cell;

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
    fn swap_exchanges_the_two_boards() {
        let mut vs = VsComputer::new(1, 0);
        vs.player.board_mut().set(3, 20, Some(Cell::die(5)));
        assert_eq!(cell_count(&vs.player), 1);
        assert_eq!(cell_count(&vs.ai), 0);

        vs.deliver(WeaponToken::Swap, Side::Player);
        // Deferred: each side installs the other's launch-time board at its next lock.
        lock(&mut vs.player);
        lock(&mut vs.ai);

        assert_eq!(cell_count(&vs.player), 0, "player received Ernie's empty board");
        assert_eq!(
            vs.ai.board().get(3, 20).map(|c| c.value()),
            Some(5),
            "Ernie received the player's board"
        );
    }

    #[test]
    fn swap_drops_bottle_and_upbyside_on_both_sides() {
        let mut vs = VsComputer::new(1, 0);
        // Arm Bottle on the player and Upbyside on Ernie via the normal flush.
        vs.player.receive_weapon(WeaponToken::Bottle);
        lock(&mut vs.player);
        vs.ai.receive_weapon(WeaponToken::Upbyside);
        lock(&mut vs.ai);
        assert!(vs.player.board().active.is_active(WeaponToken::Bottle));
        assert!(vs.ai.board().active.is_active(WeaponToken::Upbyside));

        vs.deliver(WeaponToken::Swap, Side::Player);
        // Deferred: the Bottle/Upbyside cleanup lands when each side installs the
        // swapped board at its next lock.
        lock(&mut vs.player);
        lock(&mut vs.ai);

        assert!(!vs.player.board().active.is_active(WeaponToken::Bottle), "Swap cleared Bottle");
        assert!(!vs.ai.board().active.is_active(WeaponToken::Upbyside), "Swap cleared Upbyside");
    }

    #[test]
    fn susan_exchanges_arsenals() {
        let mut vs = VsComputer::new(1, 0);
        vs.player.grant_weapon(WeaponToken::RiseUp);
        vs.player.grant_weapon(WeaponToken::Blind);
        assert_eq!(vs.ai.arsenal_token(0), -1);

        vs.deliver(WeaponToken::Susan, Side::Player);

        assert_eq!(vs.player.arsenal_token(0), -1, "player arsenal emptied");
        assert_eq!(vs.ai.arsenal_token(0), WeaponToken::RiseUp.index() as i32);
        assert_eq!(vs.ai.arsenal_token(1), WeaponToken::Blind.index() as i32);
    }

    /// Curse `side` by having the OTHER side deploy Mirror onto them, then lock
    /// so it activates (the offensive Mirror is a normal attack on the opponent).
    fn curse(vs: &mut VsComputer, side: Side) {
        vs.deliver(WeaponToken::Mirror, side.other());
        match side {
            Side::Player => lock(&mut vs.player),
            Side::Ai => lock(&mut vs.ai),
        }
        assert!(vs.game(side).weapon_active(WeaponToken::Mirror), "Mirror should be active");
    }

    #[test]
    fn launching_mirror_curses_the_opponent_not_the_launcher() {
        let mut vs = VsComputer::new(1, 0);
        vs.deliver(WeaponToken::Mirror, Side::Player); // player launches Mirror at Ernie
        lock(&mut vs.ai); // it activates on Ernie at his next lock

        assert!(vs.ai.weapon_active(WeaponToken::Mirror), "Mirror curses the opponent (Ernie)");
        assert!(!vs.player.weapon_active(WeaponToken::Mirror), "the launcher itself is not armed");
    }

    #[test]
    fn a_cursed_launchers_swap_fizzles() {
        let mut vs = VsComputer::new(1, 0);
        curse(&mut vs, Side::Player);
        vs.player.board_mut().set(3, 20, Some(Cell::die(5)));
        let (p0, a0) = (cell_count(&vs.player), cell_count(&vs.ai));

        vs.deliver(WeaponToken::Swap, Side::Player); // Swap is on the nullify list

        assert_eq!(cell_count(&vs.player), p0, "cursed Swap fizzles; player board unchanged");
        assert_eq!(cell_count(&vs.ai), a0, "Ernie's board untouched");
    }

    #[test]
    fn a_cursed_launchers_weapon_backfires_onto_themselves() {
        let mut vs = VsComputer::new(1, 0);
        curse(&mut vs, Side::Player);
        let a0 = cell_count(&vs.ai);

        // Player launches RiseUp at Ernie; cursed, it backfires onto the player.
        vs.deliver(WeaponToken::RiseUp, Side::Player);
        lock(&mut vs.player); // flush the backfired weapon onto the player

        assert_eq!(cell_count(&vs.ai), a0, "Ernie (the intended target) was spared");
        assert!(cell_count(&vs.player) >= 9, "the backfired RiseUp hit the player");
    }

    #[test]
    fn an_uncursed_spy_activates_on_the_opponent() {
        // Positive control so the D6 fizzle test below isn't vacuous.
        let mut vs = VsComputer::new(1, 0);
        vs.deliver(WeaponToken::Ames, Side::Player);
        lock(&mut vs.ai);
        assert!(vs.ai.weapon_active(WeaponToken::Ames), "Ames activates on Ernie normally");
    }

    #[test]
    fn d6_a_cursed_launchers_spy_fizzles() {
        let mut vs = VsComputer::new(1, 0);
        curse(&mut vs, Side::Player);

        // Cursed, the player's spy is one of the nullify-9 and fizzles entirely.
        vs.deliver(WeaponToken::Ames, Side::Player);
        lock(&mut vs.ai);
        lock(&mut vs.player);

        assert!(!vs.ai.weapon_active(WeaponToken::Ames), "the spy did not reach Ernie");
        assert!(!vs.player.weapon_active(WeaponToken::Ames), "nor did it self-inflict");
    }

    #[test]
    fn keating_credits_the_attacker_in_the_relay() {
        let mut vs = VsComputer::new(1, 0);
        // Give Ernie a treasury (no line clears on this empty board, so it stays).
        vs.ai.add_funds(500);
        vs.ai.take_events(); // drop the bookkeeping Scored from add_funds
        let p0 = vs.player.score().funds;

        // Player Keatings Ernie; it lands at Ernie's next lock.
        vs.deliver(WeaponToken::Keating, Side::Player);
        vs.ai.begin_drop();
        for _ in 0..400 {
            vs.ai.tick(16); // drive Ernie to a lock WITHOUT draining his events
            if vs.ai.score().funds == 0 {
                break; // Keating flushed and zeroed him
            }
        }
        assert_eq!(vs.ai.score().funds, 0, "Ernie was robbed");

        vs.relay(); // routes Ernie's FundsStolen to the attacker (player)
        assert_eq!(vs.player.score().funds, p0 + 500, "the attacker banked the seized 500");
    }
    // (The Mirror nullify-set is now owned + tested in bt_core::versus.)
}
