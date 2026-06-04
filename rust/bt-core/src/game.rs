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
use crate::cell::Cell;
use crate::constants::*;
use crate::piece::{Piece, PieceKind};
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
    /// Funds this (victim) player just lost to the opponent and that the relay
    /// must CREDIT to the attacker: Mondale's 30% cut of newly-banked funds, or
    /// Keating's full seizure. Faithful to `BTScoreManager.C` (the original
    /// reconstructs the attacker's gain from the victim's reported score; with
    /// full information at the relay we emit the exact amount instead). The
    /// attacker is always this player's opponent, so the relay routes it to
    /// "the other side" — see `VsComputer::relay`.
    FundsStolen(i64),
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
    /// Mutable board access — for the cross-player relay (Swap), sandbox/test
    /// setup, and tooling. Normal gameplay goes through the typed methods.
    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
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
            // BT_FUNDS: Mondale taxes the victim to (1 - 0.30) of funds earned;
            // the swiped 30% is credited to the attacker by the relay
            // (`FundsStolen`), faithful to BTScoreManager.C:154-202.
            let gained = if self.weapons.is_active(WeaponToken::Mondale) {
                let kept = (clear.funds as f64 * (1.0 - BT_MONDALE_RATE)) as i64;
                // CORRECTNESS over faithfulness: the attacker gains EXACTLY what the
                // victim lost (`clear.funds - kept`), so the tax CONSERVES money.
                // The 1994 original (BTScoreManager.C:154-160) reconstructed the cut
                // from the victim's already-TRUNCATED funds delta sent over the P2P
                // wire — a second independent truncation with no shared remainder,
                // which DESTROYS up to 2 funds per clear (the victim loses more than
                // the attacker gains; see `mondale_transfer_conserves_funds`). We
                // have full information at the relay, so we transfer the exact
                // remainder instead. (Diverges from the binary by <=2 funds/clear.)
                let tax = clear.funds as i64 - kept;
                if tax != 0 {
                    self.events.push(GameEvent::FundsStolen(tax));
                }
                kept
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
            // Slid off the edge in time — keep falling. Advance the game's
            // position AND the piece's own: collision/locking read self.x/y,
            // while render + land() read p.x/y, so they must stay in lockstep.
            // (Omitting the p.y sync here let a piece lock one row above where it
            // rendered — resting in mid-air; caught by the position-sync PBT.)
            self.y += self.delta_y;
            p.x = self.x;
            p.y = self.y;
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

    /// `BTGame::beginDrop` — engage fast drop and award the human hard-drop
    /// bonus (`BT_BOARD_HGT - y_`, BTGame.C:729): the further the piece still
    /// had to fall, the more it's worth.
    pub fn begin_drop(&mut self) {
        if self.engage_fast_drop() {
            self.score.score += (BT_BOARD_HGT - self.y) as i64;
        }
    }

    /// Ernie's placement scoring (`BTComputer::run`, BTComputer.C:1255). The
    /// computer engages the *same* fast drop for motion but does NOT earn the
    /// human hard-drop bonus — it banks a flat `BT_BOARD_HGT / 2` per piece,
    /// once, regardless of how far the piece fell. Keeping Ernie off the human
    /// scoring curve is why this is separate from [`Game::begin_drop`]; without
    /// it the AI inherited the full `BT_BOARD_HGT - 0 = 28` bonus every piece.
    pub fn ai_begin_drop(&mut self) {
        if self.engage_fast_drop() {
            self.score.score += (BT_BOARD_HGT / 2) as i64;
        }
    }

    /// Shared `beginDrop` motion: switch the falling piece to the fast cadence.
    /// Returns `true` only when this call *newly* engages the fast drop (so the
    /// caller awards its score bonus exactly once); `false` while paused / in
    /// the bazaar / after game over, or when fast drop is already engaged.
    fn engage_fast_drop(&mut self) -> bool {
        if self.paused || self.in_bazaar || self.phase == Phase::Over {
            return false;
        }
        self.dropping = true;
        if self.drop_time == self.fast_drop_time {
            return false;
        }
        self.drop_time = self.fast_drop_time;
        // Re-arm the drop timer for the faster cadence.
        self.drop_accum = 0;
        true
    }

    /// Soft drop: advance the piece down exactly one cell (or begin the lock
    /// slide if it can't move down). One call per tap; the caller can hold-
    /// repeat this for a fast, controlled descent. Resets the gravity clock so
    /// manual stepping isn't compounded by an immediate gravity tick.
    pub fn soft_drop(&mut self) {
        if self.paused || self.in_bazaar || self.phase != Phase::Falling {
            return;
        }
        self.drop_step();
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
    /// Whether `token`'s effect is currently active on this game.
    pub fn weapon_active(&self, token: WeaponToken) -> bool {
        self.weapons.is_active(token)
    }
    /// Lines of duration left for `token` (0 = inactive, expired, or instant).
    pub fn weapon_remaining(&self, token: WeaponToken) -> i32 {
        self.remaining[token.index()]
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

    /// Credit (or debit) this player's funds directly — the relay's hook for
    /// paying the attacker the funds a Mondale/Keating victim lost (see
    /// [`GameEvent::FundsStolen`]). Emits a `Scored` so the gain propagates to
    /// the opponent's mirror like any other funds change.
    pub fn add_funds(&mut self, amount: i64) {
        if amount == 0 {
            return;
        }
        self.score.funds += amount;
        self.events.push(GameEvent::Scored {
            score: self.score.score,
            lines: self.score.lines,
            funds: self.score.funds,
        });
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

    /// Sell `token` back in the bazaar (the "Remove" button): refund its
    /// effective price and remove one from the arsenal. Returns true on success.
    ///
    /// NB the refund is [`Self::bazaar_price`], which (like the buy price) tracks
    /// the CURRENT Carter multiplier — faithful to BTBazaar.C:458. This makes
    /// buying un-cursed and selling while Carter-cursed a deliberate +base-price
    /// arbitrage ("buy low, stack, cash out double once cursed") — a kept skill
    /// boon, NOT a bug. See `carter_buy_uncursed_sell_cursed_is_an_intentional_arbitrage_boon`.
    pub fn sell_weapon(&mut self, token: WeaponToken) -> bool {
        if !self.in_bazaar {
            return false;
        }
        if self.arsenal.sell(token) {
            self.score.funds += self.bazaar_price(token) as i64;
            true
        } else {
            false
        }
    }

    /// Leave the bazaar and resume play (`BTGame::leaveBazaar`).
    pub fn leave_bazaar(&mut self) {
        self.in_bazaar = false;
    }

    // ---- cross-player weapon primitives -----------------------------------
    // Swap / Susan / Mirror act across BOTH players, so the relay (VsComputer
    // or the online layer) drives them through these. A lone Game can't reach
    // its opponent, so the orchestration lives one level up.

    /// Force a weapon off now: revert its effect (if active) and zero its
    /// remaining duration. Used by Swap, which cancels Bottle and Upbyside on
    /// both boards (BTGame.C:494-528).
    pub fn force_weapon_off(&mut self, token: WeaponToken) {
        if self.weapons.is_active(token) {
            self.apply_weapon_off(token);
        }
        self.remaining[token.index()] = 0;
    }

    /// Swap Meet (`BTGame.C:492-534`): exchange this game's board grid with
    /// `other`'s, after both sides drop Bottle and Upbyside. Only the grid
    /// moves — active flags, durations, funds, score and the falling piece all
    /// stay with their player.
    pub fn swap_board_with(&mut self, other: &mut Game) {
        for t in [WeaponToken::Bottle, WeaponToken::Upbyside] {
            self.force_weapon_off(t);
            other.force_weapon_off(t);
        }
        self.board.swap_cells(&mut other.board);
    }

    /// Lazy Susan (`BTWeaponManager.C:104-110`): exchange arsenals between the
    /// two players.
    pub fn swap_arsenal_with(&mut self, other: &mut Game) {
        std::mem::swap(&mut self.arsenal, &mut other.arsenal);
    }

    /// Add one `token` to the arsenal directly (no bazaar / no funds). For a
    /// sandbox/debug mode and for driving weapon launches in tests. Returns
    /// false if the arsenal is full of distinct kinds.
    pub fn grant_weapon(&mut self, token: WeaponToken) -> bool {
        self.arsenal.buy(token)
    }

    // ---- cross-player serialization (online Swap / Susan / spy) ------------
    // The online layer has no shared engine, so these ship a board grid or
    // arsenal over the data channel. Each round-trips its export/import.

    /// The playfield as a flat `width*height` grid of render ids (row-major),
    /// with the falling piece overlaid; empty squares are `-2` (matching the
    /// front-end's `EMPTY` sentinel / `WasmGame::render_grid`). This is the form
    /// the canvas draws — used by the server's spy reveal (a degraded copy).
    pub fn render_ids(&self) -> Vec<i32> {
        let (w, h) = (self.board.width, self.board.height);
        let mut grid = vec![-2i32; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = self.board.get(x, y) {
                    grid[(y * w + x) as usize] = c.id();
                }
            }
        }
        if let Some(p) = self.current.as_ref() {
            for i in 0..BT_PIECE_WIDTH {
                for j in 0..BT_PIECE_HEIGHT {
                    if let Some(c) = p.cells[i][j] {
                        let gx = p.x + i as i32;
                        let gy = p.y + j as i32;
                        if gx >= 0 && gx < w && gy >= 0 && gy < h {
                            grid[(gy * w + gx) as usize] = c.id();
                        }
                    }
                }
            }
        }
        grid
    }

    /// Encode the board grid as a flat `[tag,a,b,hidden]` quad per cell,
    /// row-major (`Cell::encode`). Used to send a board to the opponent for
    /// Swap (exchange) or a spy (display).
    pub fn export_board(&self) -> Vec<i32> {
        let mut out = Vec::with_capacity((self.board.width * self.board.height * 4) as usize);
        for y in 0..self.board.height {
            for x in 0..self.board.width {
                out.extend_from_slice(&self.board.get(x, y).map(|c| c.encode()).unwrap_or([0; 4]));
            }
        }
        out
    }

    /// Replace the board grid from an `export_board` encoding. Only the cells
    /// move (Swap clears Bottle/Upbyside separately, at the relay). Wrong-length
    /// input is ignored.
    pub fn import_board(&mut self, data: &[i32]) {
        let (w, h) = (self.board.width, self.board.height);
        if data.len() != (w * h * 4) as usize {
            return;
        }
        let mut i = 0;
        for y in 0..h {
            for x in 0..w {
                let cell = Cell::decode([data[i], data[i + 1], data[i + 2], data[i + 3]]);
                self.board.set(x, y, cell);
                i += 4;
            }
        }
    }

    /// Encode the arsenal as `[token_index, quantity]` per slot (10 slots;
    /// empty = `[-1, 0]`) for Lazy Susan.
    pub fn export_arsenal(&self) -> Vec<i32> {
        let mut out = Vec::with_capacity(20);
        for i in 0..10 {
            out.push(self.arsenal.token(i).map(|t| t.index() as i32).unwrap_or(-1));
            out.push(self.arsenal.quantity(i) as i32);
        }
        out
    }

    /// Rebuild the arsenal from an `export_arsenal` encoding. Sets each slot
    /// directly so the exact layout (including holes) is preserved, and clamps
    /// quantities so a malformed/hostile peer message can't hang or overflow.
    pub fn import_arsenal(&mut self, data: &[i32]) {
        if data.len() != 20 {
            return;
        }
        let mut a = Arsenal::new();
        for slot in 0..10 {
            let token = WeaponToken::from_index(data[slot * 2]);
            let qty = data[slot * 2 + 1].clamp(0, u16::MAX as i32) as u16;
            a.set_slot(slot, token, qty);
        }
        self.arsenal = a;
    }

    // ---- full-game keyframe (client-server reconciliation) ----------------
    // Server-authoritative online play needs a COMPLETE engine snapshot the
    // client can restore and then re-simulate its unacked inputs on top of.
    // `export_board` alone is only a render view — it omits the falling piece,
    // phase/timers, the RNG + piece-manager state, and the weapon flags/pending
    // queue, all of which drive the deterministic stream. `snapshot`/`restore`
    // capture the whole `Game` as a flat `i64` codec (no serde — bt-core stays
    // dependency-free), versioned for forward-compat.

    /// Keyframe format version (bump on any layout change).
    pub const KEYFRAME_VERSION: i64 = 1;

    /// Serialize the entire game state to a flat `i64` keyframe.
    pub fn snapshot(&self) -> Vec<i64> {
        let phase = match self.phase {
            Phase::Falling => 0,
            Phase::Sliding => 1,
            Phase::Over => 2,
        };
        let mut o: Vec<i64> = vec![Self::KEYFRAME_VERSION];
        for v in [
            self.x, self.y, self.def_x, self.def_y, self.delta_y, self.left_x, self.right_x,
            self.base_drop_time, self.fast_drop_time, self.drop_time, self.slide_time,
            self.dropping as i32, self.sliding, phase,
            self.drop_accum, self.slide_accum, self.paused as i32,
            self.hatter_accum, self.slick_accum, self.slick_dir,
            self.in_bazaar as i32, self.lines_til_baz,
        ] {
            o.push(v as i64);
        }
        o.extend_from_slice(&[
            self.score.score, self.score.op_score, self.score.lines,
            self.score.op_lines, self.score.funds, self.score.op_funds,
        ]);
        o.push(self.rng.raw() as i64);
        let (kp, hap_on, broken, old_piece) = self.pieces.raw();
        for f in kp {
            o.push(f.to_bits() as i64);
        }
        o.push(hap_on as i64);
        o.push(broken as i64);
        o.push(old_piece as i64);
        for cnt in self.weapons.raw() {
            o.push(cnt as i64);
        }
        for r in self.remaining {
            o.push(r as i64);
        }
        for a in self.export_arsenal() {
            o.push(a as i64);
        }
        match &self.current {
            Some(p) => {
                o.push(1);
                o.push(p.kind.id() as i64);
                for v in [p.x, p.y, p.color, p.rot as i32, p.orientation, p.orientations, p.state] {
                    o.push(v as i64);
                }
                for col in 0..BT_PIECE_WIDTH {
                    for row in 0..BT_PIECE_HEIGHT {
                        let q = p.cells[col][row].map(|c| c.encode()).unwrap_or([0; 4]);
                        for v in q {
                            o.push(v as i64);
                        }
                    }
                }
            }
            None => o.push(0),
        }
        o.push(self.pending.len() as i64);
        for t in &self.pending {
            o.push(t.index() as i64);
        }
        for v in self.export_board() {
            o.push(v as i64);
        }
        // Board-level state beyond the cells: its own BTActive[] (consulted by
        // FallOut/Bottle/Force/Upbyside mechanics in board.rs), the Upbyside flip,
        // the computer-board flag, and the idiot bad-move latch.
        for cnt in self.board.active.raw() {
            o.push(cnt as i64);
        }
        o.push(self.board.upside as i64);
        o.push(self.board.computer as i64);
        o.push(self.board.idiot as i64);
        o.push(self.board.reason as i64);
        o
    }

    /// Restore the entire game state from a [`Self::snapshot`] keyframe. Reads
    /// into locals first and only commits if the keyframe is well-formed and
    /// fully consumed, so a malformed/truncated keyframe leaves `self` untouched
    /// and returns `false` (never panics — the cursor is bounds-checked).
    pub fn restore(&mut self, data: &[i64]) -> bool {
        let mut c = Cur { d: data, i: 0, ok: true };
        if c.next() != Self::KEYFRAME_VERSION {
            return false;
        }
        let x = c.next() as i32;
        let y = c.next() as i32;
        let def_x = c.next() as i32;
        let def_y = c.next() as i32;
        let delta_y = c.next() as i32;
        let left_x = c.next() as i32;
        let right_x = c.next() as i32;
        let base_drop_time = c.next() as i32;
        let fast_drop_time = c.next() as i32;
        let drop_time = c.next() as i32;
        let slide_time = c.next() as i32;
        let dropping = c.next() != 0;
        let sliding = c.next() as i32;
        let phase = match c.next() {
            0 => Phase::Falling,
            1 => Phase::Sliding,
            2 => Phase::Over,
            _ => return false,
        };
        let drop_accum = c.next() as i32;
        let slide_accum = c.next() as i32;
        let paused = c.next() != 0;
        let hatter_accum = c.next() as i32;
        let slick_accum = c.next() as i32;
        let slick_dir = c.next() as i32;
        let in_bazaar = c.next() != 0;
        let lines_til_baz = c.next() as i32;
        let score = Score {
            score: c.next(),
            op_score: c.next(),
            lines: c.next(),
            op_lines: c.next(),
            funds: c.next(),
            op_funds: c.next(),
        };
        let rng_state = c.next() as u64;
        let mut kp = [0.0f64; 19];
        for k in kp.iter_mut() {
            *k = f64::from_bits(c.next() as u64);
        }
        let hap_on = c.next() as i32;
        let broken = c.next() != 0;
        let old_piece = c.next() as i32;
        let mut counts = [0i32; BT_MAX_WEAPONS];
        for cnt in counts.iter_mut() {
            *cnt = c.next() as i32;
        }
        let mut remaining = [0i32; BT_MAX_WEAPONS];
        for r in remaining.iter_mut() {
            *r = c.next() as i32;
        }
        let mut arsenal_flat = [0i32; 20];
        for a in arsenal_flat.iter_mut() {
            *a = c.next() as i32;
        }
        let current = if c.next() != 0 {
            let kind = match PieceKind::from_id(c.next() as i32) {
                Some(k) => k,
                None => return false,
            };
            let px = c.next() as i32;
            let py = c.next() as i32;
            let color = c.next() as i32;
            let rot = c.next() as usize;
            let orientation = c.next() as i32;
            let orientations = c.next() as i32;
            let state = c.next() as i32;
            // Reject implausible piece geometry before it can panic a later rotate
            // (a corrupt keyframe; the server never produces these). rot is the
            // rotation sub-square side: 0 (no rotate), 3, 4, or 8.
            if !matches!(rot, 0 | 3 | 4 | 8)
                || orientations <= 0
                || orientation < 0
                || orientation >= orientations
            {
                return false;
            }
            let mut cells: [[Option<Cell>; BT_PIECE_HEIGHT]; BT_PIECE_WIDTH] =
                [[None; BT_PIECE_HEIGHT]; BT_PIECE_WIDTH];
            for col in 0..BT_PIECE_WIDTH {
                for row in 0..BT_PIECE_HEIGHT {
                    let q = [c.next() as i32, c.next() as i32, c.next() as i32, c.next() as i32];
                    cells[col][row] = Cell::decode(q);
                }
            }
            Some(Piece { kind, x: px, y: py, color, rot, orientation, orientations, state, cells })
        } else {
            None
        };
        let pending_len = c.next();
        if !(0..=1024).contains(&pending_len) {
            return false;
        }
        let mut pending = Vec::with_capacity(pending_len as usize);
        for _ in 0..pending_len {
            match WeaponToken::from_index(c.next() as i32) {
                Some(t) => pending.push(t),
                None => return false,
            }
        }
        let board_len = (self.board.width * self.board.height * 4) as usize;
        let mut board_flat = Vec::with_capacity(board_len);
        for _ in 0..board_len {
            board_flat.push(c.next() as i32);
        }
        let mut board_active = [0i32; BT_MAX_WEAPONS];
        for a in board_active.iter_mut() {
            *a = c.next() as i32;
        }
        let board_upside = c.next() != 0;
        let board_computer = c.next() != 0;
        let board_idiot = c.next() != 0;
        let board_reason = c.next() as i16;
        // Reject a malformed (short) or trailing-garbage keyframe before committing.
        if !c.ok || c.i != data.len() {
            return false;
        }

        // Commit.
        self.x = x;
        self.y = y;
        self.def_x = def_x;
        self.def_y = def_y;
        self.delta_y = delta_y;
        self.left_x = left_x;
        self.right_x = right_x;
        self.base_drop_time = base_drop_time;
        self.fast_drop_time = fast_drop_time;
        self.drop_time = drop_time;
        self.slide_time = slide_time;
        self.dropping = dropping;
        self.sliding = sliding;
        self.phase = phase;
        self.drop_accum = drop_accum;
        self.slide_accum = slide_accum;
        self.paused = paused;
        self.hatter_accum = hatter_accum;
        self.slick_accum = slick_accum;
        self.slick_dir = slick_dir;
        self.in_bazaar = in_bazaar;
        self.lines_til_baz = lines_til_baz;
        self.score = score;
        self.rng = Rng::from_raw(rng_state);
        self.pieces.set_raw(kp, hap_on, broken, old_piece);
        self.weapons.set_raw(counts);
        self.remaining = remaining;
        self.import_arsenal(&arsenal_flat);
        self.import_board(&board_flat);
        self.board.active.set_raw(board_active);
        self.board.upside = board_upside;
        self.board.computer = board_computer;
        self.board.idiot = board_idiot;
        self.board.reason = board_reason;
        self.current = current;
        self.pending = pending;
        self.events.clear();
        true
    }

    /// [`Self::snapshot`] as little-endian bytes — the wire form for the
    /// client-server keyframe. (The i64 stream includes `keep_prob` f64
    /// bit-patterns that exceed 2^53, so it can't ride as JSON numbers; bytes
    /// round-trip exactly and the transport sends them as a `Uint8Array`.)
    pub fn snapshot_bytes(&self) -> Vec<u8> {
        self.snapshot().iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    /// The keyframe as a CLIENT is allowed to see it: identical to
    /// [`Self::snapshot_bytes`] except `op_funds` (this player's mirror of the
    /// opponent's funds) is zeroed. Funds are spy-revealed in the original, so
    /// the authoritative server must not leak the opponent's funds to a client
    /// through the reconciliation keyframe. `op_funds` is display-only — the
    /// client's simulation never reads it — so zeroing it is harmless to
    /// reconciliation (a restored client just shows 0 until a spy reveals it).
    pub fn client_keyframe_bytes(&self) -> Vec<u8> {
        let mut g = self.clone();
        g.score.op_funds = 0;
        g.snapshot_bytes()
    }

    /// Restore from a [`Self::snapshot_bytes`] buffer. False if the length isn't
    /// a multiple of 8 or the keyframe is malformed (see [`Self::restore`]).
    pub fn restore_bytes(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() % 8 != 0 {
            return false;
        }
        let kf: Vec<i64> = bytes
            .chunks_exact(8)
            .map(|c| i64::from_le_bytes(c.try_into().unwrap()))
            .collect();
        self.restore(&kf)
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
            WeaponToken::Keating => {
                // "...all taken away ... and given to you." Zero the victim and
                // hand the seized funds to the attacker via the relay. NOTE: this
                // snapshots the victim's funds at flush (next lock); the original
                // snapshots `keating_ = op_funds` at launch (BTScoreManager.C:110).
                // Same net effect unless the victim banks funds in that one-piece
                // window — a known minor divergence (the port applies at lock).
                let stolen = self.score.funds;
                self.score.funds = 0;
                if stolen != 0 {
                    self.events.push(GameEvent::FundsStolen(stolen));
                }
            }
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

/// A bounds-checked forward cursor over an `i64` keyframe (see [`Game::restore`]).
/// A read past the end flips `ok` to false and yields 0, so a malformed/truncated
/// keyframe is rejected rather than panicking.
struct Cur<'a> {
    d: &'a [i64],
    i: usize,
    ok: bool,
}

impl Cur<'_> {
    fn next(&mut self) -> i64 {
        match self.d.get(self.i) {
            Some(&v) => {
                self.i += 1;
                v
            }
            None => {
                self.ok = false;
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The "blocks sit in mid-air" guarantee: `place()` (the lock-delay expiry)
    /// re-checks gravity. If the piece can still move down — e.g. it was slid
    /// over a hole during the 150ms slide window — it RESUMES FALLING instead of
    /// locking floating. So a locked piece is always supported; overhangs from a
    /// tuck are possible (classic Tetris keeps holes), but nothing locks with
    /// empty space under every cell.
    #[test]
    fn a_piece_that_can_still_fall_resumes_falling_not_locks_midair() {
        let mut g = Game::new(1);
        let y0 = g.y;
        let piece_id = g.current.as_ref().unwrap().kind.id();
        // Pretend the lock-delay is running while the fresh piece is still high
        // up with nothing under it (the "slid over a hole" situation).
        g.phase = Phase::Sliding;
        g.sliding = 1;
        assert!(
            g.current.as_ref().unwrap().can_move_to(&g.board, g.x, g.y + g.delta_y),
            "precondition: the piece can move down"
        );
        g.place(false);
        assert_eq!(g.phase, Phase::Falling, "it resumed falling");
        assert_eq!(g.y, y0 + g.delta_y, "gravity moved it down a row");
        assert_eq!(
            g.current.as_ref().map(|p| p.kind.id()),
            Some(piece_id),
            "same piece still in play — it did NOT lock or spawn a new one"
        );
        // The crux of the mid-air bug: the game's position (collision/lock) and
        // the piece's own (render/land) must stay in lockstep after the resume.
        let p = g.current.as_ref().unwrap();
        assert_eq!((g.x, g.y), (p.x, p.y), "game & piece positions stay synced");
    }

    /// While the weapons bazaar is open the whole game is frozen
    /// (`BTGame::pauseAllTimeOuts`), so player input must be inert. This guards
    /// the regression where a held drop key kept the falling piece moving while
    /// the human was supposed to be shopping.
    #[test]
    fn begin_drop_is_ignored_in_the_bazaar() {
        let mut g = Game::new(1);
        assert!(g.current_piece().is_some(), "a piece should be falling");
        assert!(!g.dropping);

        // Enter the bazaar (the synchronized weapons barrier).
        g.in_bazaar = true;
        g.begin_drop();
        assert!(!g.dropping, "begin_drop must be a no-op while the bazaar is open");

        // Leaving the bazaar re-enables fast drop.
        g.in_bazaar = false;
        g.begin_drop();
        assert!(g.dropping, "begin_drop should engage once the bazaar closes");
    }

    #[test]
    fn soft_drop_is_ignored_in_the_bazaar() {
        let mut g = Game::new(1);
        let y0 = g.y;
        g.in_bazaar = true;
        g.soft_drop();
        assert_eq!(g.y, y0, "soft_drop must not advance the piece in the bazaar");
    }

    /// A game in the bazaar is frozen: ticking the virtual clock must not move
    /// or lock the piece, change the score, or emit events.
    #[test]
    fn tick_is_frozen_in_the_bazaar() {
        let mut g = Game::new(1);
        g.in_bazaar = true;
        let y0 = g.y;
        let score0 = g.score().score;
        for _ in 0..200 {
            g.tick(16);
        }
        assert_eq!(g.y, y0, "the piece must not fall while frozen");
        assert_eq!(g.score().score, score0, "score must not change while frozen");
        assert!(g.take_events().is_empty(), "no events while frozen");
    }

    #[test]
    fn keyframe_round_trips_exactly() {
        let mut g = Game::new(0xBEEF);
        for i in 0..200 {
            match i {
                50 => g.move_left(),
                70 => g.rotate(),
                90 => g.begin_drop(),
                120 => g.receive_weapon(WeaponToken::Bottle),
                _ => {}
            }
            g.tick(16);
        }
        let snap = g.snapshot();
        let mut h = Game::new(1); // a DIFFERENT seed — restore must overwrite everything
        assert!(h.restore(&snap), "restore accepts a valid keyframe");
        assert_eq!(h.snapshot(), snap, "the restored game re-serializes identically");

        // The byte (wire) form round-trips too.
        let mut hb = Game::new(2);
        assert!(hb.restore_bytes(&g.snapshot_bytes()));
        assert_eq!(hb.snapshot(), snap, "byte-form restore matches");
        assert!(!hb.restore_bytes(&[1, 2, 3]), "a non-multiple-of-8 buffer is rejected");
    }

    #[test]
    fn keyframe_enables_deterministic_continuation() {
        // The reason this codec exists: restore a mid-game keyframe into a fresh
        // engine and the two must stay bit-identical when driven the same way —
        // including the RNG-advancing weapon that naive board-snapping can't track.
        let mut a = Game::new(0x1234);
        for i in 0..200 {
            match i {
                // Upbyside is a PERSISTENT board-active weapon: it flips the board
                // (board.upside) and sets board.active[Upbyside]. If the keyframe
                // omitted those, the restored game would fall the wrong way and
                // diverge here. RiseUp adds a garbage row (RNG-advancing).
                40 => a.receive_weapon(WeaponToken::Upbyside),
                80 => a.begin_drop(),
                110 => a.receive_weapon(WeaponToken::RiseUp),
                150 => a.begin_drop(),
                _ => {}
            }
            a.tick(16);
        }
        let mut b = Game::new(0x9999); // different seed; the keyframe overrides it
        assert!(b.restore(&a.snapshot()));

        for i in 0..300 {
            match i {
                30 => {
                    a.move_left();
                    b.move_left();
                }
                60 => {
                    a.rotate();
                    b.rotate();
                }
                100 => {
                    a.begin_drop();
                    b.begin_drop();
                }
                _ => {}
            }
            a.tick(16);
            b.tick(16);
        }
        assert_eq!(a.snapshot(), b.snapshot(), "continuation from a keyframe is deterministic");
    }

    #[test]
    fn restore_rejects_malformed_keyframes_without_mutating() {
        let good = Game::new(7).snapshot();
        let mut g = Game::new(7);
        let before = g.snapshot();

        assert!(!g.restore(&[]), "empty rejected");
        assert!(!g.restore(&good[..good.len() - 1]), "truncated rejected");
        let mut wrong_ver = good.clone();
        wrong_ver[0] = 999;
        assert!(!g.restore(&wrong_ver), "wrong version rejected");
        let mut trailing = good.clone();
        trailing.push(123);
        assert!(!g.restore(&trailing), "trailing garbage rejected");

        assert_eq!(g.snapshot(), before, "self is untouched after every rejected restore");
    }
}
