//! WebAssembly bindings exposing the BattleTris engine to the browser.
//!
//! `WasmGame` wraps [`bt_core::Game`] with a JS-facing API: fixed-step `tick`,
//! input, weapon launch / bazaar, the two-player relay surface (`receive_weapon`
//! / `receive_op_score`), structured events, and `render_grid` for the Canvas.

use bt_ai::Computer;
use bt_core::constants::{BT_PIECE_HEIGHT, BT_PIECE_WIDTH};
use bt_core::game::GameEvent;
use bt_core::weapons::{weapon_table, WeaponToken, BT_MAX_WEAPONS};
use bt_core::Game;
use wasm_bindgen::prelude::*;

/// Sentinel id for an empty square in [`WasmGame::render_grid`].
pub const EMPTY: i32 = -2;

// Event tags (paired with 3 i32 payload slots in `drain_events`).
const TAG_LOCKED: i32 = 0; // [lines, value, funds]
const TAG_WEAPON_LAUNCHED: i32 = 1; // [token, 0, 0]
const TAG_SCORED: i32 = 2; // [score, lines, funds]
const TAG_ENTER_BAZAAR: i32 = 3; // [0, 0, 0]
const TAG_AIRSLIDE: i32 = 4; // [0, 0, 0]
const TAG_GAME_OVER: i32 = 5; // [0, 0, 0]
const TAG_IDIOT: i32 = 6; // [reason, 0, 0]

#[wasm_bindgen]
pub struct WasmGame {
    inner: Game,
}

#[wasm_bindgen]
impl WasmGame {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> WasmGame {
        WasmGame { inner: Game::new(seed as u64) }
    }

    pub fn width(&self) -> i32 {
        self.inner.board().width
    }
    pub fn height(&self) -> i32 {
        self.inner.board().height
    }

    pub fn tick(&mut self, dt_ms: i32) {
        self.inner.tick(dt_ms);
    }

    pub fn move_left(&mut self) {
        self.inner.move_left();
    }
    pub fn move_right(&mut self) {
        self.inner.move_right();
    }
    pub fn rotate(&mut self) {
        self.inner.rotate();
    }
    pub fn begin_drop(&mut self) {
        self.inner.begin_drop();
    }
    /// Soft drop one cell (tap = 1; hold-repeat for fast descent).
    pub fn soft_drop(&mut self) {
        self.inner.soft_drop();
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.inner.set_paused(paused);
    }

    pub fn is_game_over(&self) -> bool {
        self.inner.is_game_over()
    }
    pub fn is_paused(&self) -> bool {
        self.inner.is_paused()
    }

    // --- score ---
    pub fn score(&self) -> i32 {
        self.inner.score().score as i32
    }
    pub fn lines(&self) -> i32 {
        self.inner.score().lines as i32
    }
    pub fn funds(&self) -> i32 {
        self.inner.score().funds as i32
    }
    pub fn op_score(&self) -> i32 {
        self.inner.score().op_score as i32
    }
    pub fn op_lines(&self) -> i32 {
        self.inner.score().op_lines as i32
    }
    pub fn op_funds(&self) -> i32 {
        self.inner.score().op_funds as i32
    }

    // --- weapons / bazaar ---
    pub fn launch_weapon(&mut self, slot: u32) {
        self.inner.launch_weapon(slot as usize);
    }
    /// Deliver a weapon the opponent launched (token = protocol index).
    pub fn receive_weapon(&mut self, token: i32) {
        if let Some(t) = WeaponToken::from_index(token) {
            self.inner.receive_weapon(t);
        }
    }
    /// Deliver the opponent's latest score (`BT_OP_SCORE`).
    pub fn receive_op_score(&mut self, score: i32, lines: i32, funds: i32) {
        self.inner
            .receive_op_score(score as i64, lines as i64, funds as i64);
    }
    pub fn is_in_bazaar(&self) -> bool {
        self.inner.is_in_bazaar()
    }
    pub fn lines_til_bazaar(&self) -> i32 {
        self.inner.lines_til_bazaar()
    }
    /// Buy a weapon by token index; returns true on success.
    pub fn buy_weapon(&mut self, token: i32) -> bool {
        match WeaponToken::from_index(token) {
            Some(t) => self.inner.buy_weapon(t),
            None => false,
        }
    }
    /// Sell a weapon back (bazaar "Remove"); refunds its price.
    pub fn sell_weapon(&mut self, token: i32) -> bool {
        match WeaponToken::from_index(token) {
            Some(t) => self.inner.sell_weapon(t),
            None => false,
        }
    }
    pub fn leave_bazaar(&mut self) {
        self.inner.leave_bazaar();
    }
    /// Arsenal slot `i`: token index, or -1 if empty.
    pub fn arsenal_token(&self, i: u32) -> i32 {
        self.inner.arsenal_token(i as usize)
    }
    pub fn arsenal_quantity(&self, i: u32) -> i32 {
        self.inner.arsenal_quantity(i as usize) as i32
    }

    /// Playfield as a flat width*height array of cell ids (piece overlaid;
    /// [`EMPTY`] = no box).
    pub fn render_grid(&self) -> Vec<i32> {
        let b = self.inner.board();
        let w = b.width;
        let h = b.height;
        let mut grid = vec![EMPTY; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = b.get(x, y) {
                    grid[(y * w + x) as usize] = c.id();
                }
            }
        }
        if let Some(p) = self.inner.current_piece() {
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

    /// Drain queued events as a flat array of `[tag, a, b, c]` quads.
    pub fn drain_events(&mut self) -> Vec<i32> {
        let mut out = Vec::new();
        for e in self.inner.take_events() {
            out.extend_from_slice(&event_quad(e));
        }
        out
    }

    /// Effective bazaar price for `token` (doubled while Carter is active).
    pub fn bazaar_price(&self, token: i32) -> i32 {
        match WeaponToken::from_index(token) {
            Some(t) => self.inner.bazaar_price(t),
            None => 0,
        }
    }
}

// --- weapon catalog (for the bazaar UI) ---------------------------------

#[wasm_bindgen]
pub fn max_weapons() -> i32 {
    BT_MAX_WEAPONS as i32
}

#[wasm_bindgen]
pub fn weapon_name(token: i32) -> String {
    match WeaponToken::from_index(token) {
        Some(t) => weapon_table()[t.index()].name.to_string(),
        None => String::new(),
    }
}

#[wasm_bindgen]
pub fn weapon_description(token: i32) -> String {
    match WeaponToken::from_index(token) {
        Some(t) => weapon_table()[t.index()].description.to_string(),
        None => String::new(),
    }
}

#[wasm_bindgen]
pub fn weapon_price(token: i32) -> i32 {
    match WeaponToken::from_index(token) {
        Some(t) => weapon_table()[t.index()].price as i32,
        None => 0,
    }
}

#[wasm_bindgen]
pub fn weapon_duration(token: i32) -> i32 {
    match WeaponToken::from_index(token) {
        Some(t) => weapon_table()[t.index()].duration as i32,
        None => 0,
    }
}

// --- shared helpers -----------------------------------------------------

/// A game's playfield as a flat width*height id grid with the piece overlaid.
fn render_grid_of(g: &Game) -> Vec<i32> {
    let b = g.board();
    let w = b.width;
    let h = b.height;
    let mut grid = vec![EMPTY; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = b.get(x, y) {
                grid[(y * w + x) as usize] = c.id();
            }
        }
    }
    if let Some(p) = g.current_piece() {
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

/// Encode a [`GameEvent`] as a `[tag, a, b, c]` quad.
fn event_quad(e: GameEvent) -> [i32; 4] {
    match e {
        GameEvent::Locked { lines, value, funds } => [TAG_LOCKED, lines, value, funds],
        GameEvent::WeaponLaunched(t) => [TAG_WEAPON_LAUNCHED, t.index() as i32, 0, 0],
        GameEvent::Scored { score, lines, funds } => {
            [TAG_SCORED, score as i32, lines as i32, funds as i32]
        }
        GameEvent::EnterBazaar => [TAG_ENTER_BAZAAR, 0, 0, 0],
        GameEvent::Idiot(reason) => [TAG_IDIOT, reason as i32, 0, 0],
        GameEvent::Airslide => [TAG_AIRSLIDE, 0, 0, 0],
        GameEvent::GameOver => [TAG_GAME_OVER, 0, 0, 0],
    }
}

// --- vs-computer (Ernie) ------------------------------------------------

const AI_PLACE_PERIOD_MS: i32 = 350;
const AI_LAUNCH_PERIOD_MS: i32 = 4000;

/// A single-tab game vs the computer opponent. Owns the player's game plus the
/// AI's game and relays weapons / scores between them internally (the original
/// `BattleTris -X` mode). Mirrors the [`WasmGame`] method names so the
/// front-end can drive either with the same code.
#[wasm_bindgen]
pub struct WasmVsComputer {
    player: Game,
    ai: Game,
    computer: Computer,
    place_accum: i32,
    launch_accum: i32,
    /// 0 = ongoing, 1 = player won, 2 = player lost.
    result: i32,
    events: Vec<i32>,
}

#[wasm_bindgen]
impl WasmVsComputer {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> WasmVsComputer {
        let mut vs = WasmVsComputer {
            player: Game::new(seed as u64),
            ai: Game::new((seed as u64) ^ 0x9E37_79B9_7F4A_7C15),
            computer: Computer::new(),
            place_accum: 0,
            launch_accum: 0,
            result: 0,
            events: Vec::new(),
        };
        if vs.ai.current_piece().is_some() {
            vs.computer.take_turn(&mut vs.ai);
        }
        vs
    }

    pub fn tick(&mut self, dt_ms: i32) {
        if self.result != 0 {
            return;
        }
        self.player.tick(dt_ms);
        self.ai_logic(dt_ms);
        self.relay();
    }

    fn ai_logic(&mut self, dt: i32) {
        self.ai.tick(dt);

        // Bazaar: greedily buy affordable weapons, then leave.
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

        // Place the current piece on a throttle so it's watchable.
        self.place_accum += dt;
        if self.place_accum >= AI_PLACE_PERIOD_MS {
            self.place_accum = 0;
            if !self.ai.is_game_over() && self.ai.current_piece().is_some() {
                self.computer.take_turn(&mut self.ai);
            }
        }

        // Periodically fire a weapon if the AI has one.
        self.launch_accum += dt;
        if self.launch_accum >= AI_LAUNCH_PERIOD_MS {
            self.launch_accum = 0;
            for slot in 0..10u32 {
                if self.ai.arsenal_token(slot as usize) >= 0 {
                    self.ai.launch_weapon(slot as usize);
                    break;
                }
            }
        }
    }

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
            self.events.extend_from_slice(&event_quad(e));
        }
        for e in self.ai.take_events() {
            match e {
                GameEvent::WeaponLaunched(t) => self.player.receive_weapon(t),
                GameEvent::Scored { score, lines, funds } => {
                    self.player.receive_op_score(score, lines, funds)
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

    // --- player API (mirrors WasmGame) ---
    pub fn width(&self) -> i32 {
        self.player.board().width
    }
    pub fn height(&self) -> i32 {
        self.player.board().height
    }
    pub fn move_left(&mut self) {
        self.player.move_left();
    }
    pub fn move_right(&mut self) {
        self.player.move_right();
    }
    pub fn rotate(&mut self) {
        self.player.rotate();
    }
    pub fn begin_drop(&mut self) {
        self.player.begin_drop();
    }
    pub fn soft_drop(&mut self) {
        self.player.soft_drop();
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.player.set_paused(paused);
    }
    pub fn is_game_over(&self) -> bool {
        self.player.is_game_over() || self.result != 0
    }
    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }
    pub fn score(&self) -> i32 {
        self.player.score().score as i32
    }
    pub fn lines(&self) -> i32 {
        self.player.score().lines as i32
    }
    pub fn funds(&self) -> i32 {
        self.player.score().funds as i32
    }
    pub fn op_score(&self) -> i32 {
        self.player.score().op_score as i32
    }
    pub fn op_lines(&self) -> i32 {
        self.player.score().op_lines as i32
    }
    pub fn op_funds(&self) -> i32 {
        self.player.score().op_funds as i32
    }
    pub fn launch_weapon(&mut self, slot: u32) {
        self.player.launch_weapon(slot as usize);
    }
    pub fn is_in_bazaar(&self) -> bool {
        self.player.is_in_bazaar()
    }
    pub fn lines_til_bazaar(&self) -> i32 {
        self.player.lines_til_bazaar()
    }
    pub fn buy_weapon(&mut self, token: i32) -> bool {
        match WeaponToken::from_index(token) {
            Some(t) => self.player.buy_weapon(t),
            None => false,
        }
    }
    pub fn sell_weapon(&mut self, token: i32) -> bool {
        match WeaponToken::from_index(token) {
            Some(t) => self.player.sell_weapon(t),
            None => false,
        }
    }
    pub fn leave_bazaar(&mut self) {
        self.player.leave_bazaar();
    }
    pub fn bazaar_price(&self, token: i32) -> i32 {
        match WeaponToken::from_index(token) {
            Some(t) => self.player.bazaar_price(t),
            None => 0,
        }
    }
    pub fn arsenal_token(&self, i: u32) -> i32 {
        self.player.arsenal_token(i as usize)
    }
    pub fn arsenal_quantity(&self, i: u32) -> i32 {
        self.player.arsenal_quantity(i as usize) as i32
    }
    pub fn render_grid(&self) -> Vec<i32> {
        render_grid_of(&self.player)
    }
    /// The AI's board (optional spectator view).
    pub fn render_ai_grid(&self) -> Vec<i32> {
        render_grid_of(&self.ai)
    }
    pub fn drain_events(&mut self) -> Vec<i32> {
        std::mem::take(&mut self.events)
    }
}
