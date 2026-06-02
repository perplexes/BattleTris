//! The single-player "vs computer" match — the original `BattleTris -X` mode.
//!
//! [`VsComputer`] owns the human player's [`Game`] plus Ernie's [`Game`] and
//! relays weapons / scores between them, applies the bazaar barrier, and
//! throttles Ernie's placement to the chosen difficulty. It is plain Rust (no
//! wasm-bindgen), so it can be driven headlessly in tests by advancing
//! [`VsComputer::tick`] over a virtual clock — see `tests/vs_computer.rs`.
//!
//! `bt-wasm`'s `WasmVsComputer` is a thin wrapper that adds the JS-facing event
//! encoding around this engine.

use crate::Computer;
use bt_core::game::GameEvent;
use bt_core::weapons::WeaponToken;
use bt_core::Game;

/// Ernie's difficulty table — the per-move delays (ms) from the original
/// `BTComputer.C` `levels[]` (Comatose … Bionic). The challenge screen's
/// "Ernie slider" picks one of these; the page exposes the same choice.
pub const AI_LEVELS: [i32; 15] = [
    4000, 3000, 2000, 1500, 1250, 1000, 750, 550, 400, 350, 300, 225, 100, 10, 0,
];

/// Ms between Ernie's weapon launches.
pub const AI_LAUNCH_PERIOD_MS: i32 = 4000;

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
    launch_accum: i32,
    /// True once Ernie has steered the current piece into its drop. `take_turn`
    /// only fires on a *fresh* piece: it ends with `begin_drop` (a fast-drop
    /// that takes several ticks to land, not an instant placement), so without
    /// this gate a short `place_period` would re-fire on the still-falling
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
        let mut vs = VsComputer {
            player: Game::new(seed),
            ai: Game::new(seed ^ 0x9E37_79B9_7F4A_7C15),
            computer: Computer::new(),
            place_period: AI_LEVELS[idx],
            place_accum: 0,
            launch_accum: 0,
            ai_committed: false,
            result: 0,
            events: Vec::new(),
        };
        if vs.ai.current_piece().is_some() {
            vs.computer.take_turn(&mut vs.ai);
            vs.ai_committed = true;
        }
        vs
    }

    /// Advance the match by `dt_ms` of virtual time.
    pub fn tick(&mut self, dt_ms: i32) {
        if self.result != 0 {
            return;
        }

        // Bazaar barrier (`BTGame` pauses ALL timeouts on `BT_START_BAZ` and only
        // resumes once BOTH sides have left — see BattleTris(1) and
        // `BTComputer::checkBazaar`). Both games enter together at every 20th
        // combined line; while the human shops, the whole match is frozen so
        // Ernie can't rack up free real-time turns. Ernie does its one-shot
        // shopping on entry, then waits for the human's DONE (which the page
        // signals via `leave_bazaar`).
        if self.player.is_in_bazaar() || self.ai.is_in_bazaar() {
            if self.ai.is_in_bazaar() {
                let mut bought = 0;
                for t in WeaponToken::ALL {
                    if bought >= 5 {
                        break;
                    }
                    if self.ai.buy_weapon(t) {
                        bought += 1;
                    }
                }
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
        // difficulty's per-move delay). Only steer a *fresh* piece — one we
        // haven't already committed to a drop (see `ai_committed`).
        self.place_accum += dt;
        if !self.ai_committed && self.place_accum >= self.place_period {
            self.place_accum = 0;
            if !self.ai.is_game_over() && self.ai.current_piece().is_some() {
                self.computer.take_turn(&mut self.ai);
                self.ai_committed = true;
            }
        }

        // Periodically fire a weapon if the AI has one.
        self.launch_accum += dt;
        if self.launch_accum >= AI_LAUNCH_PERIOD_MS {
            self.launch_accum = 0;
            for slot in 0..10usize {
                if self.ai.arsenal_token(slot) >= 0 {
                    self.ai.launch_weapon(slot);
                    break;
                }
            }
        }
    }

    /// Wire weapons / scores between the two games and capture player-side
    /// events for rendering. `result` is latched the first time either side
    /// tops out (1 = player won, 2 = player lost).
    fn relay(&mut self) {
        for e in self.player.take_events() {
            match e {
                GameEvent::WeaponLaunched(t) => self.ai.receive_weapon(t),
                GameEvent::Scored { score, lines, funds } => {
                    self.ai.receive_op_score(score, lines, funds)
                }
                GameEvent::GameOver => self.result = 2,
                _ => {}
            }
            self.events.push(e);
        }
        for e in self.ai.take_events() {
            match e {
                GameEvent::WeaponLaunched(t) => self.player.receive_weapon(t),
                GameEvent::Scored { score, lines, funds } => {
                    self.player.receive_op_score(score, lines, funds)
                }
                // The AI's piece settled — ready a fresh one, and restart the
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

    /// 0 = ongoing, 1 = player won, 2 = player lost.
    pub fn result(&self) -> i32 {
        self.result
    }

    /// The human player's game (read-only — for rendering / inspection).
    pub fn player(&self) -> &Game {
        &self.player
    }

    /// The human player's game (mutable — the host drives input through this).
    pub fn player_mut(&mut self) -> &mut Game {
        &mut self.player
    }

    /// Ernie's game (read-only — the optional spectator view).
    pub fn ai(&self) -> &Game {
        &self.ai
    }

    /// Take the queued player-side events (for the host to render). Cleared.
    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }
}
