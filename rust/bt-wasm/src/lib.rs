//! WebAssembly bindings exposing the BattleTris engine to the browser.
//!
//! `WasmGame` wraps [`bt_core::Game`] with a JS-facing API: fixed-step `tick`,
//! input, weapon launch / bazaar, the two-player relay surface (`receive_weapon`
//! / `receive_op_score`), structured events, and `render_grid` for the Canvas.

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
            let quad = match e {
                GameEvent::Locked { lines, value, funds } => [TAG_LOCKED, lines, value, funds],
                GameEvent::WeaponLaunched(t) => [TAG_WEAPON_LAUNCHED, t.index() as i32, 0, 0],
                GameEvent::Scored { score, lines, funds } => {
                    [TAG_SCORED, score as i32, lines as i32, funds as i32]
                }
                GameEvent::EnterBazaar => [TAG_ENTER_BAZAAR, 0, 0, 0],
                GameEvent::Airslide => [TAG_AIRSLIDE, 0, 0, 0],
                GameEvent::GameOver => [TAG_GAME_OVER, 0, 0, 0],
            };
            out.extend_from_slice(&quad);
        }
        out
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
