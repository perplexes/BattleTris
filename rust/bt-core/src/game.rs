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

use crate::board::Board;
use crate::constants::*;
use crate::piece::Piece;
use crate::piece_manager::PieceManager;
use crate::rng::Rng;

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
        if self.paused || self.phase == Phase::Over || dt_ms <= 0 {
            return;
        }
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
        // BT_SLIDE_TIME * (1 - BTActive[NO_SLIDE]); no weapons yet -> full delay.
        self.slide_time = BT_SLIDE_TIME;
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

            // Lock the piece into the board (fills cells + idiot detection).
            p.land(&mut self.board);
            self.board.flush_idiot();

            let clear = self.board.check_lines();
            self.score.lines += clear.lines as i64;
            self.score.funds += clear.funds as i64;
            self.events.push(GameEvent::Locked {
                lines: clear.lines,
                value: clear.value,
                funds: clear.funds,
            });

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
}
