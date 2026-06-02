//! The single-player game state machine — a faithful, deterministic port of the
//! falling/sliding/locking core of `BTGame` (`usr/src/game/BTGame.C`).
//!
//! The original is driven by Xt timeouts (`BT_DROP_TIMEOUT`, `BT_SLIDE_TIMEOUT`,
//! …). For a headless, reproducible engine we replace the real-time timer loop
//! with an explicit [`Game::tick`] that advances a virtual clock by `dt_ms`.
//! Each frame the WASM front-end calls `tick` and feeds input events.
//!
//! This first cut covers the heart of the game — spawn → fall → slide → lock →
//! clear lines → award funds → spawn → death — for a single board. Weapons, the
//! bazaar, and the two-player relay layer on top of this via the score/funds
//! economy already modeled here (see the `op_*` score fields and [`GameEvent`]).

use crate::arsenal::Arsenal;
use crate::board::Board;
use crate::constants::*;
use crate::piece::Piece;
use crate::piece_manager::PieceManager;
use crate::rng::Rng;
use crate::weapons::{weapon_table, ActiveFlags, WeaponToken, BT_MAX_WEAPONS};

/// `BTScore` — the per-player scoreboard (`usr/src/game/BTScore.H`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Score {
    /// Hard-drop bonus points (`rep_.score_`).
    pub score: i64,
    pub op_score: i64,
    /// Total lines cleared (`rep_.lines_`).
    pub lines: i64,
    pub op_lines: i64,
    /// Funds earned from die/happy values × line multipliers (`rep_.funds_`).
    pub funds: i64,
    pub op_funds: i64,
}

/// Whether a drop tick or a slide tick is currently armed (the original keeps
/// `BT_DROP_TIMEOUT` and `BT_SLIDE_TIMEOUT` as separate timers; only one is
/// "live" for the falling piece at a time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Falling,
    Sliding,
    Over,
}

/// Events emitted by the engine for the host (front-end / two-player relay).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameEvent {
    /// A piece locked and `lines` rows cleared for `funds` (0 lines = just a
    /// lock). `value` is the summed pip value of the cleared rows.
    Locked { lines: i32, value: i32, funds: i32 },
    /// An "airslide" was performed (`BT_AIRSLIDE`).
    Airslide,
    /// This player launched a weapon — the relay must deliver it to the
    /// opponent (where it becomes `BT_WPN_ON`). Mirrors `BT_WPN_LAUNCH`.
    WeaponLaunched(WeaponToken),
    /// This player's score changed — the relay sends it to the opponent as
    /// `BT_OP_SCORE` (drives Lawyers' Delite, taxes, the bazaar trigger).
    Scored { score: i64, lines: i64, funds: i64 },
    /// Combined lines crossed a multiple of 20 — open the weapons bazaar
    /// (`BT_START_BAZ`).
    EnterBazaar,
    /// An "idiot" signal after a lock (`BT_IDIOT`): bad move / near death /
    /// missed smiley (see `BT_BAD_MOVE` / `BT_NEAR_DEATH` / `BT_MISSED_SMILEY`).
    Idiot(i16),
    /// The player topped out (`BT_GAME_OVER`).
    GameOver,
}

/// A single player's game.
#[derive(Clone, Debug)]
pub struct Game {
    board: Board,
    pieces: PieceManager,
    rng: Rng,
    score: Score,

    current: Option<Piece>,
    x: i32,
    y: i32,

    // Spawn / movement frame (flipped by Upbyside; constant for now).
    def_x: i32,
    def_y: i32,
    delta_y: i32,
    left_x: i32,
    right_x: i32,

    // Drop timing (ms).
    base_drop_time: i32,
    fast_drop_time: i32,
    drop_time: i32,
    slide_time: i32,

    dropping: bool, // `drop_` — fast drop engaged
    sliding: i32,   // `sliding_` — slide counter (airslide bookkeeping)

    phase: Phase,
    drop_accum: i32,
    slide_accum: i32,
    paused: bool,

    // --- weapons (BTWeaponManager) ---
    /// Active-weapon flags affecting this player (`BTActive[]`).
    weapons: ActiveFlags,
    /// Remaining duration in lines per weapon (`remaining_[]`).
    remaining: [i32; BT_MAX_WEAPONS],
    /// This player's arsenal.
    arsenal: Arsenal,
    /// Weapons received from the opponent, applied at the next piece lock
    /// (`BTCommManager::weapq_` flushed in `place`).
    pending: Vec<WeaponToken>,
    /// Auto-rotate (Mad Hatter) / auto-slide (Slick Willy) sub-timers.
    hatter_accum: i32,
    slick_accum: i32,
    slick_dir: i32,
    /// In the weapons bazaar (game frozen).
    in_bazaar: bool,
    /// Lines remaining until the next bazaar (combined player+opponent lines).
    lines_til_baz: i32,

    events: Vec<GameEvent>,
}

impl Game {
    /// Start a new game seeded deterministically. Mirrors `BTGame::reset` +
    /// `BTGame::startGame`: installs defaults and spawns the first piece.
    pub fn new(seed: u64) -> Game {
        let mut g = Game {
            board: Board::standard(false),
            pieces: PieceManager::new(),
            rng: Rng::new(seed),
            score: Score::default(),
            current: None,
            x: BT_DEFAULT_X,
            y: BT_DEFAULT_Y,
            def_x: BT_DEFAULT_X,
            def_y: BT_DEFAULT_Y,
            delta_y: 1,
            left_x: -1,
            right_x: 1,
            base_drop_time: BT_DROP_TIME,
            fast_drop_time: BT_FAST_DROP_TIME,
            drop_time: BT_DROP_TIME,
            slide_time: BT_SLIDE_TIME,
            dropping: false,
            sliding: 0,
            phase: Phase::Falling,
            drop_accum: 0,
            slide_accum: 0,
            paused: false,
            weapons: ActiveFlags::new(),
            remaining: [0; BT_MAX_WEAPONS],
            arsenal: Arsenal::new(),
            pending: Vec::new(),
            hatter_accum: 0,
            slick_accum: 0,
            slick_dir: 0,
            in_bazaar: false,
            lines_til_baz: BT_LINES_TIL_BAZ,
            events: Vec::new(),
        };
        g.start_game();
        g
    }

    // ---- queries -----------------------------------------------------------

    pub fn board(&self) -> &Board {
        &self.board
    }
    pub fn score(&self) -> Score {
        self.score
    }
    pub fn current_piece(&self) -> Option<&Piece> {
        self.current.as_ref()
    }
    /// Current piece origin on the board (`x_`, `y_`).
    pub fn piece_pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }
    pub fn is_game_over(&self) -> bool {
        self.phase == Phase::Over
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }
    /// Drain queued events (host consumes these each frame).
    pub fn take_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    // ---- lifecycle ---------------------------------------------------------

    /// `BTGame::startGame` — spawn the first piece and arm the drop timer.
    fn start_game(&mut self) {
        self.x = self.def_x;
        self.y = self.def_y;
        self.spawn();
    }

    /// Create and place the next piece. Mirrors the spawn tail of
    /// `BTGame::place`: `x_ = def_x_ - rot_/2`, and a failed initial placement
    /// is a top-out (`BT_GAME_OVER`). Returns true on success.
    fn spawn(&mut self) -> bool {
        self.x = self.def_x;
        self.y = self.def_y;
        let mut p = self.pieces.create(&mut self.rng, self.def_x, self.def_y);
        self.x = self.def_x - (p.rot as i32) / 2;
        if !p.move_to(&self.board, self.x, self.y) {
            self.current = None;
            self.phase = Phase::Over;
            self.events.push(GameEvent::GameOver);
            return false;
        }
        self.current = Some(p);
        self.dropping = false;
        self.drop_time = self.base_drop_time;
        self.phase = Phase::Falling;
        self.drop_accum = 0;
        true
    }

    // ---- clock -------------------------------------------------------------

    /// Advance the virtual clock by `dt_ms`, firing drop/slide steps as their
    /// intervals elapse. No-op while paused or after game over.
    pub fn tick(&mut self, dt_ms: i32) {
        if self.paused || self.in_bazaar || self.phase == Phase::Over || dt_ms <= 0 {
            return;
        }
        self.tick_weapons(dt_ms);
        match self.phase {
            Phase::Falling => {
                self.drop_accum += dt_ms;
                // Guard against a zero/negative interval.
                let step = self.drop_time.max(1);
                while self.phase == Phase::Falling && self.drop_accum >= step {
                    self.drop_accum -= step;
                    self.drop_step();
                }
            }
            Phase::Sliding => {
                self.slide_accum += dt_ms;
                let step = self.slide_time.max(0);
                // slide_time can be 0 (No Slide) -> lock immediately.
                loop {
                    if self.phase != Phase::Sliding {
                        break;
                    }
                    if step > 0 && self.slide_accum < step {
                        break;
                    }
                    if step > 0 {
                        self.slide_accum -= step;
                    }
                    self.place(false);
                    if step == 0 {
                        break;
                    }
                }
            }
            Phase::Over => {}
        }
    }

    /// `BTGame::drop` — move the piece down one row, or begin a slide if blocked.
    fn drop_step(&mut self) {
        if let Some(mut p) = self.current.take() {
            if !p.move_to(&self.board, self.x, self.y + self.delta_y) {
                self.current = Some(p);
                self.start_slide();
            } else {
                self.y += self.delta_y;
                self.current = Some(p);
            }
        }
    }

    /// `BTGame::startSlide` — switch to the slide timer (the lock delay).
    fn start_slide(&mut self) {
        self.sliding = 1;
        // BTGame::startSlide: BT_SLIDE_TIME * (1 - BTActive[NO_SLIDE]) — i.e. 0
        // (instant lock) while No Slide is active.
        self.slide_time = BT_SLIDE_TIME * (1 - self.weapons.is_active(WeaponToken::NoSlide) as i32);
        self.phase = Phase::Sliding;
        self.slide_accum = 0;
    }

    /// `BTGame::place` — slide-expiry handler: lock the piece (and spawn the
    /// next) if it still can't move down, otherwise resume falling.
    fn place(&mut self, force: bool) {
        let mut p = match self.current.take() {
            Some(p) => p,
            None => return,
        };

        let can_down = p.can_move_to(&self.board, self.x, self.y + self.delta_y);
        if !can_down || force {
            // Airslide: a fast drop that slid into place without being able to
            // move back up (`drop_ && sliding_ <= 1 && !canMoveTo(x,y-delta)`).
            if self.dropping
                && self.sliding <= 1
                && !p.can_move_to(&self.board, self.x, self.y - self.delta_y)
            {
                self.events.push(GameEvent::Airslide);
            }

            // Lock the piece into the board (fills cells + idiot bad-move flag).
            p.land(&mut self.board);

            let clear = self.board.check_lines();
            self.score.lines += clear.lines as i64;
            // BT_FUNDS: Mondale taxes the victim to (1 - 0.30) of funds earned.
            let gained = if self.weapons.is_active(WeaponToken::Mondale) {
                (clear.funds as f64 * (1.0 - BT_MONDALE_RATE)) as i64
            } else {
                clear.funds as i64
            };
            self.score.funds += gained;
            self.events.push(GameEvent::Locked {
                lines: clear.lines,
                value: clear.value,
                funds: clear.funds,
            });

            // flushIdiot AFTER checkLines (a cleared line un-flags "idiot";
            // near-death / missed-smiley are set by checkLines itself).
            if let Some(reason) = self.board.flush_idiot() {
                self.events.push(GameEvent::Idiot(reason));
            }

            // BT_LINE: count down active-weapon durations; BT_FUNDS/SCORE +
            // bazaar trigger; then publish our score for the opponent.
            if clear.lines > 0 {
                self.tick_durations(clear.lines);
                self.update_bazaar();
            }
            self.events.push(GameEvent::Scored {
                score: self.score.score,
                lines: self.score.lines,
                funds: self.score.funds,
            });

            // flushWeapons: apply weapons the opponent launched at us.
            self.flush_pending();

            // Next piece (or top-out).
            self.spawn();
        } else {
            // Slid off the edge in time — keep falling.
            self.y += self.delta_y;
            self.current = Some(p);
            self.phase = Phase::Falling;
            self.drop_accum = 0;
        }
        self.sliding = 0;
    }

    // ---- input -------------------------------------------------------------

    /// `BTGame::moveLeft`.
    pub fn move_left(&mut self) {
        if self.paused || self.phase == Phase::Over {
            return;
        }
        if let Some(mut p) = self.current.take() {
            if p.move_to(&self.board, self.x + self.left_x, self.y) {
                if self.sliding != 0 {
                    self.sliding += 1;
                }
                self.x += self.left_x;
            }
            self.current = Some(p);
        }
    }

    /// `BTGame::moveRight`.
    pub fn move_right(&mut self) {
        if self.paused || self.phase == Phase::Over {
            return;
        }
        if let Some(mut p) = self.current.take() {
            if p.move_to(&self.board, self.x + self.right_x, self.y) {
                if self.sliding != 0 {
                    self.sliding += 1;
                }
                self.x += self.right_x;
            }
            self.current = Some(p);
        }
    }

    /// `BTGame::rotate`.
    pub fn rotate(&mut self) {
        if self.paused || self.phase == Phase::Over {
            return;
        }
        if let Some(mut p) = self.current.take() {
            p.rotate(&self.board, false);
            self.current = Some(p);
        }
    }

    /// `BTGame::beginDrop` — engage fast drop and award the hard-drop bonus.
    pub fn begin_drop(&mut self) {
        if self.paused || self.phase == Phase::Over {
            return;
        }
        self.dropping = true;
        if self.drop_time == self.fast_drop_time {
            return;
        }
        self.drop_time = self.fast_drop_time;
        self.score.score += (BT_BOARD_HGT - self.y) as i64;
        // Re-arm the drop timer for the faster cadence.
        self.drop_accum = 0;
    }

    /// `BTGame::pause` (local toggle; no network send).
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    // ---- weapons & two-player relay ---------------------------------------

    pub fn is_in_bazaar(&self) -> bool {
        self.in_bazaar
    }
    pub fn lines_til_bazaar(&self) -> i32 {
        self.lines_til_baz
    }
    /// Weapon token in arsenal slot `i`, as its protocol index (or -1 if empty).
    pub fn arsenal_token(&self, i: usize) -> i32 {
        self.arsenal.token(i).map(|t| t.index() as i32).unwrap_or(-1)
    }
    pub fn arsenal_quantity(&self, i: usize) -> u16 {
        self.arsenal.quantity(i)
    }

    /// `BTWeaponManager::launchWeapon` — fire arsenal slot `slot` (0-based) at
    /// the opponent (emits [`GameEvent::WeaponLaunched`]).
    pub fn launch_weapon(&mut self, slot: usize) {
        if self.in_bazaar || self.phase == Phase::Over {
            return;
        }
        if let Some(tok) = self.arsenal.token(slot) {
            self.arsenal.use_slot(slot);
            self.events.push(GameEvent::WeaponLaunched(tok));
        }
    }

    /// A weapon launched by the opponent arrives here; applied at the next
    /// piece lock (`BTCommManager::weapq_` / `flushWeapons`).
    pub fn receive_weapon(&mut self, token: WeaponToken) {
        self.pending.push(token);
    }

    /// `BT_OP_SCORE` — the opponent's score changed. Updates the mirror, applies
    /// Lawyers' Delite, and advances the bazaar trigger.
    pub fn receive_op_score(&mut self, op_score: i64, op_lines: i64, op_funds: i64) {
        let old_op_lines = self.score.op_lines;
        self.score.op_score = op_score;
        self.score.op_funds = op_funds;
        if self.weapons.is_active(WeaponToken::Lawyers) {
            for _ in 0..(op_lines - old_op_lines).max(0) {
                self.board.insert_line(&mut self.rng);
            }
        }
        self.score.op_lines = op_lines;
        self.update_bazaar();
    }

    /// Buy `token` in the bazaar; honors Carter (price doubling). Returns true
    /// on success.
    pub fn buy_weapon(&mut self, token: WeaponToken) -> bool {
        if !self.in_bazaar {
            return false;
        }
        let mut price = weapon_table()[token.index()].price as i64;
        if self.weapons.is_active(WeaponToken::Carter) {
            price *= 2;
        }
        if self.score.funds < price {
            return false;
        }
        if self.arsenal.buy(token) {
            self.score.funds -= price;
            true
        } else {
            false
        }
    }

    /// Leave the bazaar and resume play (`BTGame::leaveBazaar`).
    pub fn leave_bazaar(&mut self) {
        self.in_bazaar = false;
    }

    /// Effective bazaar price for `token` — doubled while Carter is active
    /// (the original displays and charges the doubled price).
    pub fn bazaar_price(&self, token: WeaponToken) -> i32 {
        let p = weapon_table()[token.index()].price as i32;
        if self.weapons.is_active(WeaponToken::Carter) {
            p * 2
        } else {
            p
        }
    }

    /// `BTGame::receive(BT_WPN_ON)` + the per-subsystem effects — activate a
    /// weapon on THIS (victim) player.
    fn apply_weapon_on(&mut self, token: WeaponToken) {
        // BTActive[token] = 1 (boolean), remaining_ += duration (accumulates).
        self.weapons.set(token, true);
        self.remaining[token.index()] += weapon_table()[token.index()].duration as i32;

        self.board.set_active(token, true);
        self.board.apply_weapon(token, &mut self.rng);
        self.pieces.weapon_on(token);

        match token {
            WeaponToken::Upbyside => {
                self.def_y = BT_BOARD_HGT - 4;
                self.delta_y = -1;
                self.left_x = 1;
                self.right_x = -1;
            }
            WeaponToken::Speedy => {
                self.base_drop_time >>= 1;
                if self.drop_time != self.fast_drop_time {
                    self.drop_time = self.base_drop_time.max(1);
                }
            }
            WeaponToken::Meadow => {
                self.fast_drop_time <<= 1;
                self.base_drop_time <<= 1;
                if self.drop_time != self.fast_drop_time {
                    self.drop_time = self.base_drop_time;
                }
            }
            WeaponToken::Keating => self.score.funds = 0,
            WeaponToken::Reagan => self.score.funds = -self.score.funds,
            _ => {}
        }
    }

    /// `BT_WPN_OFF` — a weapon's duration expired; revert its effect.
    fn apply_weapon_off(&mut self, token: WeaponToken) {
        self.weapons.set(token, false);
        self.board.revert_weapon(token);
        self.board.set_active(token, false);
        self.pieces.weapon_off(token);

        match token {
            WeaponToken::Upbyside => {
                self.def_x = BT_DEFAULT_X;
                self.def_y = BT_DEFAULT_Y;
                self.delta_y = 1;
                self.left_x = -1;
                self.right_x = 1;
            }
            WeaponToken::Speedy => self.base_drop_time <<= 1,
            WeaponToken::Meadow => {
                self.base_drop_time >>= 1;
                self.fast_drop_time >>= 1;
            }
            _ => {}
        }
    }

    /// `BTWeaponManager::receive(BT_LINE)` — count active-weapon durations down
    /// by the lines just cleared; expire any that hit zero.
    fn tick_durations(&mut self, lines: i32) {
        for i in 0..BT_MAX_WEAPONS {
            if self.remaining[i] == 0 {
                continue;
            }
            self.remaining[i] = (self.remaining[i] - lines).max(0);
            if self.remaining[i] == 0 {
                if let Some(tok) = WeaponToken::from_index(i as i32) {
                    self.apply_weapon_off(tok);
                }
            }
        }
    }

    /// Apply weapons the opponent launched (queued via [`Self::receive_weapon`]).
    fn flush_pending(&mut self) {
        let pend: Vec<WeaponToken> = self.pending.drain(..).collect();
        for tok in pend {
            self.apply_weapon_on(tok);
        }
    }

    /// Recompute the bazaar countdown from combined lines; fire on crossing.
    fn update_bazaar(&mut self) {
        let combined = self.score.op_lines + self.score.lines;
        let new_til = BT_LINES_TIL_BAZ - (combined.rem_euclid(BT_LINES_TIL_BAZ as i64)) as i32;
        if new_til > self.lines_til_baz {
            self.in_bazaar = true;
            self.events.push(GameEvent::EnterBazaar);
        }
        self.lines_til_baz = new_til;
    }

    /// Mad Hatter (auto-rotate) / Slick Willy (auto-slide) sub-timers.
    fn tick_weapons(&mut self, dt: i32) {
        if self.weapons.is_active(WeaponToken::Hatter) {
            self.hatter_accum += dt;
            while self.hatter_accum >= 20 {
                self.hatter_accum -= 20;
                self.rotate_internal();
            }
        }
        // Slick is suspended during hard-drop and the slide lock (BTGame removes
        // the slick timeout in beginDrop/startSlide, re-arming after spawn).
        if self.weapons.is_active(WeaponToken::Slick)
            && self.phase == Phase::Falling
            && !self.dropping
        {
            self.slick_accum += dt;
            while self.slick_accum >= 20 {
                self.slick_accum -= 20;
                self.slick_step();
            }
        }
    }

    fn rotate_internal(&mut self) {
        if let Some(mut p) = self.current.take() {
            p.rotate(&self.board, false);
            self.current = Some(p);
        }
    }

    fn slick_step(&mut self) {
        if let Some(mut p) = self.current.take() {
            let dir = if self.slick_dir == 0 { self.left_x } else { self.right_x };
            if p.move_to(&self.board, self.x + dir, self.y) {
                self.x += dir;
            } else {
                self.slick_dir ^= 1;
            }
            self.current = Some(p);
        }
    }
}
