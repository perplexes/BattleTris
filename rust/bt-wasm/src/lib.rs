//! WebAssembly bindings exposing the BattleTris engine to the browser.
//!
//! `WasmGame` wraps [`bt_core::Game`] with a JS-facing API: fixed-step `tick`,
//! input, weapon launch / bazaar, the two-player relay surface (`receive_weapon`
//! / `receive_op_score`), structured events, and `render_grid` for the Canvas.

use bt_ai::VsComputer;
use bt_core::constants::{BT_PIECE_HEIGHT, BT_PIECE_WIDTH};
use bt_core::game::GameEvent;
use bt_core::weapons::{weapon_table, WeaponToken, BT_MAX_WEAPONS};
use bt_core::Game;
use bt_replay::{Input, Mode, Recorder, Replay, ReplayPlayer, VersusReplay, VersusReplayPlayer};
use wasm_bindgen::prelude::*;

/// Sentinel id for an empty square in [`WasmGame::render_grid`].
pub const EMPTY: i32 = -2;

/// The fixed timestep (ms) the engine is advanced with. The front-end reads this
/// via [`fixed_dt`] and drives an accumulator loop at this rate, so play (and
/// every recording) is deterministic regardless of `requestAnimationFrame`
/// jitter. Recordings replay bit-exact only when stepped at this same rate.
pub const FIXED_DT_MS: i32 = 16;

/// The engine build recordings are stamped with — the `git` short SHA passed in
/// at compile time (`BT_GIT_SHA`), or "dev" for local builds.
const ENGINE_SHA: &str = match option_env!("BT_GIT_SHA") {
    Some(s) => s,
    None => "dev",
};

/// The canonical fixed timestep (ms) the host must tick at.
#[wasm_bindgen]
pub fn fixed_dt() -> i32 {
    FIXED_DT_MS
}

// Event tags (paired with 3 i32 payload slots in `drain_events`).
const TAG_LOCKED: i32 = 0; // [lines, value, funds]
const TAG_WEAPON_LAUNCHED: i32 = 1; // [token, 0, 0]
const TAG_SCORED: i32 = 2; // [score, lines, funds]
const TAG_ENTER_BAZAAR: i32 = 3; // [0, 0, 0]
const TAG_AIRSLIDE: i32 = 4; // [0, 0, 0]
const TAG_GAME_OVER: i32 = 5; // [0, 0, 0]
const TAG_IDIOT: i32 = 6; // [reason, 0, 0]
const TAG_FUNDS_STOLEN: i32 = 7; // [amount, 0, 0] — credit the attacker (online relay)

#[wasm_bindgen]
pub struct WasmGame {
    inner: Game,
    rec: Recorder,
}

#[wasm_bindgen]
impl WasmGame {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> WasmGame {
        // Practice / 2-player share this wrapper; replays of a 2-player side are
        // self-contained because `receive_weapon` / `receive_op_score` are
        // recorded too (the runner treats Practice and VsPlayer identically).
        WasmGame {
            inner: Game::new(seed as u64),
            rec: Recorder::new(seed, Mode::Practice, None, FIXED_DT_MS, ENGINE_SHA),
        }
    }

    pub fn width(&self) -> i32 {
        self.inner.board().width
    }
    pub fn height(&self) -> i32 {
        self.inner.board().height
    }

    pub fn tick(&mut self, dt_ms: i32) {
        self.inner.tick(dt_ms);
        self.rec.on_tick();
    }

    pub fn move_left(&mut self) {
        self.inner.move_left();
        self.rec.record(Input::MoveLeft);
    }
    pub fn move_right(&mut self) {
        self.inner.move_right();
        self.rec.record(Input::MoveRight);
    }
    pub fn rotate(&mut self) {
        self.inner.rotate();
        self.rec.record(Input::Rotate);
    }
    pub fn begin_drop(&mut self) {
        self.inner.begin_drop();
        self.rec.record(Input::BeginDrop);
    }
    /// Soft drop one cell (tap = 1; hold-repeat for fast descent).
    pub fn soft_drop(&mut self) {
        self.inner.soft_drop();
        self.rec.record(Input::SoftDrop);
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.inner.set_paused(paused);
        self.rec.record(Input::SetPaused(paused));
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
        self.rec.record(Input::LaunchWeapon(slot));
    }
    /// Deliver a weapon the opponent launched (token = protocol index).
    pub fn receive_weapon(&mut self, token: i32) {
        if let Some(t) = WeaponToken::from_index(token) {
            self.inner.receive_weapon(t);
            self.rec.record(Input::ReceiveWeapon(token));
        }
    }
    /// Deliver the opponent's latest score (`BT_OP_SCORE`).
    pub fn receive_op_score(&mut self, score: i32, lines: i32, funds: i32) {
        self.inner
            .receive_op_score(score as i64, lines as i64, funds as i64);
        self.rec.record(Input::ReceiveOpScore {
            score: score as i64,
            lines: lines as i64,
            funds: funds as i64,
        });
    }
    pub fn is_in_bazaar(&self) -> bool {
        self.inner.is_in_bazaar()
    }
    pub fn lines_til_bazaar(&self) -> i32 {
        self.inner.lines_til_bazaar()
    }
    /// Credit funds taxed/seized from the opponent (online Mondale/Keating): the
    /// victim relays its `FundsStolen` amount and the attacker banks it here.
    pub fn add_funds(&mut self, amount: i32) {
        self.inner.add_funds(amount as i64);
        self.rec.record(Input::AddFunds(amount as i64));
    }
    /// Restore the full authoritative game state from a server keyframe (the
    /// byte form from `Game::snapshot_bytes`), for client-server reconciliation:
    /// the client overwrites its predicted state, then re-applies its unacked
    /// inputs. Returns false on a malformed keyframe (state left untouched).
    pub fn restore_keyframe(&mut self, bytes: Vec<u8>) -> bool {
        self.inner.restore_bytes(&bytes)
    }
    /// The full game state as a keyframe (byte form) — for debugging / parity
    /// checks against the server's authoritative snapshot.
    pub fn snapshot_keyframe(&self) -> Vec<u8> {
        self.inner.snapshot_bytes()
    }
    /// Whether weapon `token` is currently active on this game (drives the
    /// online Mirror reflect/nullify check).
    pub fn weapon_active(&self, token: i32) -> bool {
        WeaponToken::from_index(token).map_or(false, |t| self.inner.weapon_active(t))
    }
    /// Lines of duration left on weapon `token` (0 = inactive/expired/instant).
    /// Used by the in-game debug overlay to show active-weapon countdowns.
    pub fn weapon_remaining(&self, token: i32) -> i32 {
        WeaponToken::from_index(token).map_or(0, |t| self.inner.weapon_remaining(t))
    }
    /// Force a weapon off (online Swap clears Bottle/Upbyside on both sides).
    pub fn force_weapon_off(&mut self, token: i32) {
        if let Some(t) = WeaponToken::from_index(token) {
            self.inner.force_weapon_off(t);
        }
    }
    /// Cross-player board/arsenal transfer for online Swap, Susan, and spies.
    pub fn export_board(&self) -> Vec<i32> {
        self.inner.export_board()
    }
    pub fn import_board(&mut self, data: Vec<i32>) {
        self.inner.import_board(&data);
    }
    pub fn export_arsenal(&self) -> Vec<i32> {
        self.inner.export_arsenal()
    }
    pub fn import_arsenal(&mut self, data: Vec<i32>) {
        self.inner.import_arsenal(&data);
    }
    /// Buy a weapon by token index; returns true on success.
    pub fn buy_weapon(&mut self, token: i32) -> bool {
        match WeaponToken::from_index(token) {
            Some(t) => {
                let ok = self.inner.buy_weapon(t);
                self.rec.record(Input::BuyWeapon(token));
                ok
            }
            None => false,
        }
    }
    /// Sell a weapon back (bazaar "Remove"); refunds its price.
    pub fn sell_weapon(&mut self, token: i32) -> bool {
        match WeaponToken::from_index(token) {
            Some(t) => {
                let ok = self.inner.sell_weapon(t);
                self.rec.record(Input::SellWeapon(token));
                ok
            }
            None => false,
        }
    }
    pub fn leave_bazaar(&mut self) {
        self.inner.leave_bazaar();
        self.rec.record(Input::LeaveBazaar);
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

    /// The recording of this game so far, as JSON — the trace for a bug report
    /// or a saved replay. Re-running it on the same engine build reproduces the
    /// game exactly.
    pub fn export_replay(&self) -> String {
        self.rec.to_json()
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
        GameEvent::FundsStolen(amount) => [TAG_FUNDS_STOLEN, amount as i32, 0, 0],
    }
}

// --- vs-computer (Ernie) ------------------------------------------------

/// A single-tab game vs the computer opponent (Ernie). A thin wasm-facing
/// wrapper over [`bt_ai::VsComputer`]: it owns the player + AI match engine and
/// adds the JS event encoding. Mirrors the [`WasmGame`] method names so the
/// front-end can drive either with the same code. The match logic (bazaar
/// barrier, difficulty throttle, relay, win detection) lives in `bt-ai` and is
/// covered by headless tests there (`bt-ai/tests/vs_computer.rs`).
#[wasm_bindgen]
pub struct WasmVsComputer {
    inner: VsComputer,
    rec: Recorder,
}

#[wasm_bindgen]
impl WasmVsComputer {
    /// `level` indexes Ernie's difficulty table (0 = Comatose … 14 = Bionic),
    /// mirroring the original's Ernie-difficulty slider; out-of-range clamps.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, level: u32) -> WasmVsComputer {
        WasmVsComputer {
            inner: VsComputer::new(seed as u64, level as usize),
            // Only the human's inputs are recorded — Ernie is regenerated from
            // the seed + level on replay.
            rec: Recorder::new(seed, Mode::VsComputer, Some(level), FIXED_DT_MS, ENGINE_SHA),
        }
    }

    pub fn tick(&mut self, dt_ms: i32) {
        self.inner.tick(dt_ms);
        self.rec.on_tick();
    }

    /// The recording of this match so far, as JSON.
    pub fn export_replay(&self) -> String {
        self.rec.to_json()
    }

    /// 0 = ongoing, 1 = player won, 2 = player lost.
    pub fn result(&self) -> i32 {
        self.inner.result()
    }

    // --- player API (mirrors WasmGame) ---
    pub fn width(&self) -> i32 {
        self.inner.player().board().width
    }
    pub fn height(&self) -> i32 {
        self.inner.player().board().height
    }
    pub fn move_left(&mut self) {
        self.inner.player_mut().move_left();
        self.rec.record(Input::MoveLeft);
    }
    pub fn move_right(&mut self) {
        self.inner.player_mut().move_right();
        self.rec.record(Input::MoveRight);
    }
    pub fn rotate(&mut self) {
        self.inner.player_mut().rotate();
        self.rec.record(Input::Rotate);
    }
    pub fn begin_drop(&mut self) {
        self.inner.player_mut().begin_drop();
        self.rec.record(Input::BeginDrop);
    }
    pub fn soft_drop(&mut self) {
        self.inner.player_mut().soft_drop();
        self.rec.record(Input::SoftDrop);
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.inner.player_mut().set_paused(paused);
        self.rec.record(Input::SetPaused(paused));
    }
    pub fn is_game_over(&self) -> bool {
        self.inner.player().is_game_over() || self.inner.result() != 0
    }
    pub fn is_paused(&self) -> bool {
        self.inner.player().is_paused()
    }
    pub fn score(&self) -> i32 {
        self.inner.player().score().score as i32
    }
    pub fn lines(&self) -> i32 {
        self.inner.player().score().lines as i32
    }
    pub fn funds(&self) -> i32 {
        self.inner.player().score().funds as i32
    }
    pub fn op_score(&self) -> i32 {
        self.inner.player().score().op_score as i32
    }
    pub fn op_lines(&self) -> i32 {
        self.inner.player().score().op_lines as i32
    }
    pub fn op_funds(&self) -> i32 {
        self.inner.player().score().op_funds as i32
    }
    pub fn launch_weapon(&mut self, slot: u32) {
        self.inner.player_mut().launch_weapon(slot as usize);
        self.rec.record(Input::LaunchWeapon(slot));
    }
    /// Test/debug: give the player funds directly (e2e pre-stocking).
    pub fn add_funds(&mut self, amount: i32) {
        self.inner.player_mut().add_funds(amount as i64);
    }
    /// Test/debug: set the player's arsenal directly — used by the e2e test to
    /// pre-stock weapons against Ernie without playing to the bazaar.
    pub fn import_arsenal(&mut self, data: Vec<i32>) {
        self.inner.player_mut().import_arsenal(&data);
    }
    pub fn is_in_bazaar(&self) -> bool {
        self.inner.player().is_in_bazaar()
    }
    pub fn lines_til_bazaar(&self) -> i32 {
        self.inner.player().lines_til_bazaar()
    }
    pub fn buy_weapon(&mut self, token: i32) -> bool {
        match WeaponToken::from_index(token) {
            Some(t) => {
                let ok = self.inner.player_mut().buy_weapon(t);
                self.rec.record(Input::BuyWeapon(token));
                ok
            }
            None => false,
        }
    }
    pub fn sell_weapon(&mut self, token: i32) -> bool {
        match WeaponToken::from_index(token) {
            Some(t) => {
                let ok = self.inner.player_mut().sell_weapon(t);
                self.rec.record(Input::SellWeapon(token));
                ok
            }
            None => false,
        }
    }
    pub fn leave_bazaar(&mut self) {
        self.inner.player_mut().leave_bazaar();
        self.rec.record(Input::LeaveBazaar);
    }
    pub fn bazaar_price(&self, token: i32) -> i32 {
        match WeaponToken::from_index(token) {
            Some(t) => self.inner.player().bazaar_price(t),
            None => 0,
        }
    }
    pub fn arsenal_token(&self, i: u32) -> i32 {
        self.inner.player().arsenal_token(i as usize)
    }
    pub fn arsenal_quantity(&self, i: u32) -> i32 {
        self.inner.player().arsenal_quantity(i as usize) as i32
    }
    pub fn render_grid(&self) -> Vec<i32> {
        render_grid_of(self.inner.player())
    }
    /// The AI's board (optional spectator view).
    pub fn render_ai_grid(&self) -> Vec<i32> {
        render_grid_of(self.inner.ai())
    }
    pub fn drain_events(&mut self) -> Vec<i32> {
        let mut out = Vec::new();
        for e in self.inner.drain_events() {
            out.extend_from_slice(&event_quad(e));
        }
        out
    }
}

// --- replay playback ----------------------------------------------------

/// Plays back a recorded [`Replay`] in the browser — the engine behind the
/// replay library's `/replay/:id` page. Deterministic: it reconstructs the game
/// from the seed and re-applies the recorded inputs, so playback is bit-for-bit
/// identical to the original game (Ernie is regenerated for vs-computer).
#[wasm_bindgen]
pub struct WasmReplayPlayer {
    inner: ReplayPlayer,
}

#[wasm_bindgen]
impl WasmReplayPlayer {
    /// Build from a replay's JSON; throws if it doesn't parse.
    pub fn from_json(json: &str) -> Result<WasmReplayPlayer, JsValue> {
        let replay =
            Replay::from_json(json).map_err(|e| JsValue::from_str(&format!("invalid replay: {e}")))?;
        Ok(WasmReplayPlayer { inner: ReplayPlayer::new(replay) })
    }

    /// Advance one tick; returns false once the recording is exhausted.
    pub fn step(&mut self) -> bool {
        self.inner.step()
    }
    /// Jump to an absolute tick (the seek bar). Backward seeks rebuild + replay.
    pub fn seek(&mut self, tick: u32) {
        self.inner.seek(tick);
    }
    pub fn tick_index(&self) -> u32 {
        self.inner.tick_index()
    }
    pub fn tick_count(&self) -> u32 {
        self.inner.replay().tick_count
    }
    /// 0 = ongoing, 1 = player won, 2 = player lost (vs-computer replays).
    pub fn result(&self) -> i32 {
        self.inner.result()
    }
    /// "Practice" | "VsComputer" | "VsPlayer".
    pub fn mode(&self) -> String {
        format!("{:?}", self.inner.replay().mode)
    }
    pub fn seed(&self) -> u32 {
        self.inner.replay().seed
    }
    pub fn engine_sha(&self) -> String {
        self.inner.replay().engine_sha.clone()
    }
    pub fn has_ai(&self) -> bool {
        self.inner.ai().is_some()
    }

    // --- rendering / stats (mirror WasmGame so the page reuses its draw code) ---
    pub fn width(&self) -> i32 {
        self.inner.player().board().width
    }
    pub fn height(&self) -> i32 {
        self.inner.player().board().height
    }
    pub fn render_grid(&self) -> Vec<i32> {
        render_grid_of(self.inner.player())
    }
    /// Ernie's board (empty for non-vs-computer replays).
    pub fn render_ai_grid(&self) -> Vec<i32> {
        match self.inner.ai() {
            Some(g) => render_grid_of(g),
            None => Vec::new(),
        }
    }
    pub fn score(&self) -> i32 {
        self.inner.player().score().score as i32
    }
    pub fn lines(&self) -> i32 {
        self.inner.player().score().lines as i32
    }
    pub fn funds(&self) -> i32 {
        self.inner.player().score().funds as i32
    }
    pub fn op_score(&self) -> i32 {
        self.inner.player().score().op_score as i32
    }
    pub fn op_lines(&self) -> i32 {
        self.inner.player().score().op_lines as i32
    }
    pub fn lines_til_bazaar(&self) -> i32 {
        self.inner.player().lines_til_bazaar()
    }
    pub fn is_in_bazaar(&self) -> bool {
        self.inner.player().is_in_bazaar()
    }
    pub fn arsenal_token(&self, i: u32) -> i32 {
        self.inner.player().arsenal_token(i as usize)
    }
    pub fn arsenal_quantity(&self, i: u32) -> i32 {
        self.inner.player().arsenal_quantity(i as usize) as i32
    }
}

// --- online (server-authoritative) match playback -----------------------

/// Plays back a recorded online match — the deterministic two-board
/// [`VersusReplay`] the server stores. Re-runs a `bt_core::Versus` from the two
/// seeds + the recorded input stream, so the whole match (both boards, every
/// weapon / tax / bazaar / spy) reproduces exactly.
#[wasm_bindgen]
pub struct WasmVersusReplayPlayer {
    inner: VersusReplayPlayer,
    /// Weapon launches that fired in the most recent `step()`, as `(side, token)`
    /// pairs (side 0 = A, 1 = B). Captured BEFORE the launch is applied (so the
    /// slot still holds the weapon), then cleared on the next step. Drives the
    /// playback event log.
    launches: Vec<(u8, i32)>,
}

#[wasm_bindgen]
impl WasmVersusReplayPlayer {
    pub fn from_json(json: &str) -> Result<WasmVersusReplayPlayer, JsValue> {
        let replay = VersusReplay::from_json(json)
            .map_err(|e| JsValue::from_str(&format!("invalid versus replay: {e}")))?;
        Ok(WasmVersusReplayPlayer { inner: VersusReplayPlayer::new(replay), launches: Vec::new() })
    }

    pub fn step(&mut self) -> bool {
        // Capture this tick's weapon launches before they apply: resolve each
        // launched slot to its weapon token from the side's CURRENT arsenal (the
        // launch consumes the slot, so we must read it first).
        self.launches.clear();
        let tick = self.inner.tick_index();
        let slots: Vec<(u8, usize)> = self
            .inner
            .replay()
            .frames
            .iter()
            .filter(|f| f.tick == tick)
            .filter_map(|f| match f.input {
                bt_replay::Input::LaunchWeapon(slot) => Some((f.side, slot as usize)),
                _ => None,
            })
            .collect();
        for (side, slot) in slots {
            let token = self.inner.game(side == 0).arsenal_token(slot);
            if token >= 0 {
                self.launches.push((side, token));
            }
        }
        self.inner.step()
    }

    /// Weapon launches from the most recent `step()`, flat `[side, tokenIndex, …]`
    /// (side 0 = A, 1 = B). Empty on a step with no launches. The viewer appends
    /// these to a scrolling event log ("A launched <weapon>").
    pub fn recent_launches(&self) -> Vec<i32> {
        let mut out = Vec::with_capacity(self.launches.len() * 2);
        for (side, token) in &self.launches {
            out.push(*side as i32);
            out.push(*token);
        }
        out
    }
    pub fn seek(&mut self, tick: u32) {
        self.inner.seek(tick);
    }
    pub fn tick_index(&self) -> u32 {
        self.inner.tick_index()
    }
    pub fn tick_count(&self) -> u32 {
        self.inner.replay().tick_count
    }
    /// 0 = ongoing, 1 = A won, 2 = B won.
    pub fn result(&self) -> i32 {
        self.inner.result()
    }
    pub fn engine_sha(&self) -> String {
        self.inner.replay().engine_sha.clone()
    }
    pub fn width(&self) -> i32 {
        self.inner.game(true).board().width
    }
    pub fn height(&self) -> i32 {
        self.inner.game(true).board().height
    }
    /// Side A's board as a render-id grid (piece overlaid, empty = -2).
    pub fn render_a(&self) -> Vec<i32> {
        self.inner.game(true).render_ids()
    }
    /// Side B's board.
    pub fn render_b(&self) -> Vec<i32> {
        self.inner.game(false).render_ids()
    }
    pub fn score_a(&self) -> i32 {
        self.inner.game(true).score().score as i32
    }
    pub fn lines_a(&self) -> i32 {
        self.inner.game(true).score().lines as i32
    }
    pub fn score_b(&self) -> i32 {
        self.inner.game(false).score().score as i32
    }
    pub fn lines_b(&self) -> i32 {
        self.inner.game(false).score().lines as i32
    }

    // ── HUD: funds, bazaar, arsenal, active effects (per side) ───────────────
    pub fn funds_a(&self) -> i32 {
        self.inner.game(true).score().funds as i32
    }
    pub fn funds_b(&self) -> i32 {
        self.inner.game(false).score().funds as i32
    }
    pub fn lines_til_bazaar_a(&self) -> i32 {
        self.inner.game(true).lines_til_bazaar()
    }
    pub fn lines_til_bazaar_b(&self) -> i32 {
        self.inner.game(false).lines_til_bazaar()
    }
    pub fn in_bazaar_a(&self) -> bool {
        self.inner.game(true).is_in_bazaar()
    }
    pub fn in_bazaar_b(&self) -> bool {
        self.inner.game(false).is_in_bazaar()
    }

    /// Side's arsenal as a flat [token0, qty0, token1, qty1, …] of 10 slots
    /// (token = -1 for an empty slot). Mirrors the playfield arsenal panel.
    pub fn arsenal_a(&self) -> Vec<i32> {
        arsenal_pairs(self.inner.game(true))
    }
    pub fn arsenal_b(&self) -> Vec<i32> {
        arsenal_pairs(self.inner.game(false))
    }

    /// Active effects on the side, as a flat [tokenIndex, linesRemaining, …]
    /// for every weapon whose effect is currently in play (remaining > 0).
    pub fn effects_a(&self) -> Vec<i32> {
        active_effects(self.inner.game(true))
    }
    pub fn effects_b(&self) -> Vec<i32> {
        active_effects(self.inner.game(false))
    }
}

/// Flatten a game's 10 arsenal slots into [token, qty] pairs (token = -1 empty).
fn arsenal_pairs(g: &bt_core::Game) -> Vec<i32> {
    let mut out = Vec::with_capacity(20);
    for i in 0..10usize {
        out.push(g.arsenal_token(i));
        out.push(g.arsenal_quantity(i) as i32);
    }
    out
}

/// Collect [tokenIndex, linesRemaining] pairs for every active weapon effect.
fn active_effects(g: &bt_core::Game) -> Vec<i32> {
    let mut out = Vec::new();
    for tok in bt_core::WeaponToken::ALL {
        if g.weapon_active(tok) {
            out.push(tok.index() as i32);
            out.push(g.weapon_remaining(tok));
        }
    }
    out
}

#[cfg(test)]
mod recording_tests {
    //! End-to-end check that the *wrapper-level* recording (the path the browser
    //! actually drives) round-trips exactly. The wasm-bindgen types are plain
    //! Rust, so they run fine on the host.
    use super::*;

    #[test]
    fn wasm_game_export_replays_exactly() {
        let seed = 0x00AB_CDEF;
        let mut g = WasmGame::new(seed);
        for t in 0..400u32 {
            match t {
                3 => g.move_left(),
                4 => g.move_left(),
                9 => g.rotate(),
                14 => g.begin_drop(),
                60 => g.move_right(),
                95 => g.rotate(),
                130 => g.begin_drop(),
                220 => g.move_left(),
                260 => g.begin_drop(),
                _ => {}
            }
            g.tick(FIXED_DT_MS);
        }
        let live = g.render_grid();

        let replay = Replay::from_json(&g.export_replay()).expect("replay parses");
        assert_eq!(replay.dt_ms, FIXED_DT_MS);
        let mut p = ReplayPlayer::new(replay);
        p.run_to_end();

        assert_eq!(render_grid_of(p.player()), live, "wrapper recording must replay exactly");
    }

    #[test]
    fn wasm_vs_computer_export_replays_exactly() {
        let seed = 4242u32;
        let level = 9u32;
        let mut g = WasmVsComputer::new(seed, level);
        for t in 0..1200u32 {
            match t {
                5 => g.move_left(),
                11 => g.rotate(),
                20 => g.begin_drop(),
                90 => g.move_right(),
                160 => g.begin_drop(),
                _ => {}
            }
            g.tick(FIXED_DT_MS);
        }
        let live_player = g.render_grid();
        let live_ai = g.render_ai_grid();
        let live_result = g.result();

        let replay = Replay::from_json(&g.export_replay()).expect("replay parses");
        let mut p = ReplayPlayer::new(replay);
        p.run_to_end();

        assert_eq!(render_grid_of(p.player()), live_player, "human board must match");
        assert_eq!(render_grid_of(p.ai().unwrap()), live_ai, "Ernie's board must match");
        assert_eq!(p.result(), live_result, "match result must match");
    }

    #[test]
    fn replay_player_seek_is_deterministic() {
        let seed = 4242u32;
        let mut g = WasmVsComputer::new(seed, 9);
        for t in 0..800u32 {
            match t {
                5 => g.move_left(),
                20 => g.begin_drop(),
                90 => g.rotate(),
                160 => g.begin_drop(),
                _ => {}
            }
            g.tick(FIXED_DT_MS);
        }
        let json = g.export_replay();
        let mk = || WasmReplayPlayer::from_json(&json).ok().expect("replay parses");
        let total = mk().tick_count();

        // Reference end state, reached by stepping.
        let mut a = mk();
        while a.step() {}
        let end = a.render_grid();
        let end_ai = a.render_ai_grid();

        // Seeking straight to the end matches.
        let mut b = mk();
        b.seek(total);
        assert_eq!(b.render_grid(), end, "seek-to-end matches stepping");
        assert_eq!(b.render_ai_grid(), end_ai);

        // Jumping around (including backward) and landing on the end is consistent.
        let mut c = mk();
        c.seek(total / 2);
        c.seek(10);
        c.seek(total);
        assert_eq!(c.render_grid(), end, "backward+forward seek is consistent");
        assert_eq!(c.tick_index(), total);
    }
}
