//! WebAssembly bindings exposing the BattleTris engine to the browser.
//!
//! `WasmGame` wraps [`bt_core::Game`] (the faithful, deterministic game loop)
//! with a small JS-facing API: a fixed-step `tick`, input methods, score
//! getters, and `render_grid` — a flat `width*height` array of cell ids (with
//! the falling piece overlaid) for the Canvas front-end to draw.

use bt_core::constants::{BT_PIECE_HEIGHT, BT_PIECE_WIDTH};
use bt_core::game::GameEvent;
use bt_core::Game;
use wasm_bindgen::prelude::*;

/// Sentinel id for an empty square in [`WasmGame::render_grid`]. Real ids are
/// `0..=29` (and `-1` for a hidden/Twilight box); `-2` means "draw background".
pub const EMPTY: i32 = -2;

/// Event codes returned by [`WasmGame::drain_events`].
pub const EVENT_LOCKED: i32 = 1;
pub const EVENT_AIRSLIDE: i32 = 2;
pub const EVENT_GAME_OVER: i32 = 3;

#[wasm_bindgen]
pub struct WasmGame {
    inner: Game,
}

#[wasm_bindgen]
impl WasmGame {
    /// Create a new game seeded deterministically.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> WasmGame {
        WasmGame { inner: Game::new(seed as u64) }
    }

    /// Board width in cells.
    pub fn width(&self) -> i32 {
        self.inner.board().width
    }

    /// Board height in cells.
    pub fn height(&self) -> i32 {
        self.inner.board().height
    }

    /// Advance the virtual clock by `dt_ms` milliseconds.
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

    pub fn score(&self) -> i32 {
        self.inner.score().score as i32
    }
    pub fn lines(&self) -> i32 {
        self.inner.score().lines as i32
    }
    pub fn funds(&self) -> i32 {
        self.inner.score().funds as i32
    }

    /// The playfield as a flat `width*height` array (row-major, `y*width + x`)
    /// of cell ids, with the falling piece overlaid. [`EMPTY`] = no box.
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

    /// Drain queued game events as a flat array of [`EVENT_*`] codes.
    pub fn drain_events(&mut self) -> Vec<i32> {
        self.inner
            .take_events()
            .into_iter()
            .map(|e| match e {
                GameEvent::Locked { .. } => EVENT_LOCKED,
                GameEvent::Airslide => EVENT_AIRSLIDE,
                GameEvent::GameOver => EVENT_GAME_OVER,
            })
            .collect()
    }
}
