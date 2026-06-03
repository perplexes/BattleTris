//! Deterministic recording + playback for BattleTris.
//!
//! The engine is fully deterministic — [`bt_core::Game`] is seeded (drand48) and
//! advanced by an explicit `tick(dt)` clock, and Ernie ([`bt_ai`]) is a pure
//! function of the seed. So a complete recording of a game is just the seed plus
//! the timestamped sequence of player inputs:
//!
//! ```text
//! { seed, mode, ai_level, dt_ms, engine_sha, frames: [(tick, Input), …] }
//! ```
//!
//! Re-running a fresh engine with the same seed and re-applying those inputs at
//! the same tick boundaries reproduces the game bit-for-bit. That single object
//! triples as: the debugging trace behind the "report a bug" button, the content
//! of the replay library, and a deterministic test case for the headless harness
//! ("does this bug still reproduce on commit X?").
//!
//! Faithful replay requires a **fixed timestep** — the host must advance the
//! engine in constant `dt_ms` steps (an accumulator loop), not with wall-clock
//! frame deltas. [`ReplayPlayer`] does exactly that.
//!
//! ## Recording vs trace fidelity
//!
//! This is a *seed replay*: it records only the inputs and regenerates everything
//! else (gravity, Ernie's moves, RNG) by re-running the engine. It is therefore
//! faithful only on the **same engine build** — which is the point for debugging:
//! replay the same inputs on a different `engine_sha` to see where behaviour
//! diverged. `engine_sha` is recorded so playback can flag a mismatch.

use bt_ai::VsComputer;
use bt_core::game::GameEvent;
use bt_core::weapons::WeaponToken;
use bt_core::Game;
use serde::{Deserialize, Serialize};

/// Bump when the on-disk format changes incompatibly.
pub const REPLAY_VERSION: u32 = 1;

/// One state-mutating action the host can apply to a player's [`Game`]. These
/// mirror the input surface of the wasm wrappers (`WasmGame` / `WasmVsComputer`).
///
/// `ReceiveWeapon` / `ReceiveOpScore` are relay messages from an opponent — only
/// used for two-player recordings, where the opponent is an external process and
/// its effects on *this* side must be captured as inputs. In vs-computer mode
/// Ernie is regenerated from the seed, so those never appear.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Input {
    MoveLeft,
    MoveRight,
    Rotate,
    BeginDrop,
    /// Ernie's flat-scored hard drop (`Game::ai_begin_drop`, BTComputer.C:1255).
    /// Distinct from the human `BeginDrop` (which scores `BT_BOARD_HGT - y`) so
    /// an AI-driven recording — e.g. a fuzz repro — replays bit-for-bit.
    AiDrop,
    SoftDrop,
    LaunchWeapon(u32),
    BuyWeapon(i32),
    SellWeapon(i32),
    LeaveBazaar,
    SetPaused(bool),
    ReceiveWeapon(i32),
    ReceiveOpScore { score: i64, lines: i64, funds: i64 },
}

impl Input {
    /// Apply this input to a player's game (ignoring no-op return values).
    pub fn apply_to_game(&self, g: &mut Game) {
        match self {
            Input::MoveLeft => g.move_left(),
            Input::MoveRight => g.move_right(),
            Input::Rotate => g.rotate(),
            Input::BeginDrop => g.begin_drop(),
            Input::AiDrop => g.ai_begin_drop(),
            Input::SoftDrop => g.soft_drop(),
            Input::LaunchWeapon(slot) => g.launch_weapon(*slot as usize),
            Input::BuyWeapon(t) => {
                if let Some(tok) = WeaponToken::from_index(*t) {
                    g.buy_weapon(tok);
                }
            }
            Input::SellWeapon(t) => {
                if let Some(tok) = WeaponToken::from_index(*t) {
                    g.sell_weapon(tok);
                }
            }
            Input::LeaveBazaar => g.leave_bazaar(),
            Input::SetPaused(p) => g.set_paused(*p),
            Input::ReceiveWeapon(t) => {
                if let Some(tok) = WeaponToken::from_index(*t) {
                    g.receive_weapon(tok);
                }
            }
            Input::ReceiveOpScore { score, lines, funds } => {
                g.receive_op_score(*score, *lines, *funds)
            }
        }
    }
}

/// Which game a recording came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Practice,
    VsComputer,
    VsPlayer,
}

/// One recorded input, stamped with the tick index it was applied at (the number
/// of engine ticks already executed when it landed).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub tick: u32,
    pub input: Input,
}

/// A complete, self-contained recording of one player's game.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    pub version: u32,
    pub seed: u32,
    pub mode: Mode,
    /// Ernie's difficulty (vs-computer only).
    pub ai_level: Option<u32>,
    /// The fixed timestep the engine was advanced with.
    pub dt_ms: i32,
    /// The engine build this was recorded against (`git` short SHA, or "dev").
    pub engine_sha: String,
    /// Total engine ticks elapsed at the end of the recording.
    pub tick_count: u32,
    pub frames: Vec<Frame>,
    /// Optional human label (e.g. a weapon-showcase name). Absent in older
    /// recordings, so it defaults to `None` on deserialize.
    #[serde(default)]
    pub title: Option<String>,
}

impl Replay {
    /// Serialize to JSON (cannot fail for this plain data).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Parse from JSON.
    pub fn from_json(s: &str) -> Result<Replay, serde_json::Error> {
        serde_json::from_str(s)
    }
}

/// Records inputs against a virtual tick clock as a game is played. The host
/// embeds one of these in its game wrapper, calls [`Recorder::record`] from each
/// input method and [`Recorder::on_tick`] after each engine tick, then exports
/// with [`Recorder::to_json`].
#[derive(Clone, Debug)]
pub struct Recorder {
    seed: u32,
    mode: Mode,
    ai_level: Option<u32>,
    dt_ms: i32,
    engine_sha: String,
    tick: u32,
    frames: Vec<Frame>,
    title: Option<String>,
}

impl Recorder {
    pub fn new(seed: u32, mode: Mode, ai_level: Option<u32>, dt_ms: i32, engine_sha: &str) -> Recorder {
        Recorder {
            seed,
            mode,
            ai_level,
            dt_ms,
            engine_sha: engine_sha.to_string(),
            tick: 0,
            frames: Vec::new(),
            title: None,
        }
    }

    /// Attach a human label to the recording (shown in the replay library).
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    /// Stamp `input` at the current tick. Call this whenever an input is applied
    /// to the live game.
    pub fn record(&mut self, input: Input) {
        self.frames.push(Frame { tick: self.tick, input });
    }

    /// Advance the recording clock. Call once per engine tick (after the tick).
    pub fn on_tick(&mut self) {
        self.tick = self.tick.saturating_add(1);
    }

    /// Number of ticks recorded so far.
    pub fn tick(&self) -> u32 {
        self.tick
    }

    /// Number of inputs recorded so far.
    pub fn input_count(&self) -> usize {
        self.frames.len()
    }

    pub fn to_replay(&self) -> Replay {
        Replay {
            version: REPLAY_VERSION,
            seed: self.seed,
            mode: self.mode,
            ai_level: self.ai_level,
            dt_ms: self.dt_ms,
            engine_sha: self.engine_sha.clone(),
            tick_count: self.tick,
            frames: self.frames.clone(),
            title: self.title.clone(),
        }
    }

    pub fn to_json(&self) -> String {
        self.to_replay().to_json()
    }
}

/// The engine being replayed — a lone game (practice / one PvP side) or a full
/// vs-computer match (so Ernie is regenerated from the seed).
enum Engine {
    Single(Game),
    Vs(VsComputer),
}

/// Drives a [`Replay`] over the engine at its recorded fixed timestep. Step it
/// tick-by-tick (for scrubbable playback) or run it to the end (headless).
pub struct ReplayPlayer {
    engine: Engine,
    replay: Replay,
    executed: u32,
    cursor: usize,
}

impl ReplayPlayer {
    pub fn new(replay: Replay) -> ReplayPlayer {
        let seed = replay.seed as u64;
        let engine = match replay.mode {
            Mode::Practice | Mode::VsPlayer => Engine::Single(Game::new(seed)),
            Mode::VsComputer => {
                Engine::Vs(VsComputer::new(seed, replay.ai_level.unwrap_or(0) as usize))
            }
        };
        ReplayPlayer { engine, replay, executed: 0, cursor: 0 }
    }

    /// Advance exactly one engine tick: apply every input stamped at the current
    /// tick, then tick the engine. Returns false once the recording is exhausted.
    pub fn step(&mut self) -> bool {
        if self.executed >= self.replay.tick_count {
            return false;
        }
        while self.cursor < self.replay.frames.len()
            && self.replay.frames[self.cursor].tick == self.executed
        {
            let input = self.replay.frames[self.cursor].input.clone();
            self.apply(&input);
            self.cursor += 1;
        }
        match &mut self.engine {
            Engine::Single(g) => g.tick(self.replay.dt_ms),
            Engine::Vs(vs) => vs.tick(self.replay.dt_ms),
        }
        // Drain events so a long replay doesn't accumulate them unbounded; the
        // relay (vs-computer) already happens inside `VsComputer::tick`.
        match &mut self.engine {
            Engine::Single(g) => {
                let _: Vec<GameEvent> = g.take_events();
            }
            Engine::Vs(vs) => {
                let _ = vs.drain_events();
            }
        }
        self.executed += 1;
        true
    }

    fn apply(&mut self, input: &Input) {
        match &mut self.engine {
            Engine::Single(g) => input.apply_to_game(g),
            Engine::Vs(vs) => input.apply_to_game(vs.player_mut()),
        }
    }

    /// Run the whole recording to its end.
    pub fn run_to_end(&mut self) {
        while self.step() {}
    }

    /// Jump to tick `target` (clamped to `tick_count`). Seeking backward rebuilds
    /// from the seed and fast-forwards — cheap, since the engine runs thousands
    /// of ticks per millisecond. This is what a scrubber/seek bar calls.
    pub fn seek(&mut self, target: u32) {
        let target = target.min(self.replay.tick_count);
        if target < self.executed {
            *self = ReplayPlayer::new(self.replay.clone());
        }
        while self.executed < target && self.step() {}
    }

    /// Ticks executed so far.
    pub fn tick_index(&self) -> u32 {
        self.executed
    }

    /// The player's game (read-only) — for rendering / inspection.
    pub fn player(&self) -> &Game {
        match &self.engine {
            Engine::Single(g) => g,
            Engine::Vs(vs) => vs.player(),
        }
    }

    /// Ernie's game, in vs-computer replays.
    pub fn ai(&self) -> Option<&Game> {
        match &self.engine {
            Engine::Vs(vs) => Some(vs.ai()),
            Engine::Single(_) => None,
        }
    }

    /// vs-computer match result (0 ongoing, 1 player won, 2 player lost), else 0.
    pub fn result(&self) -> i32 {
        match &self.engine {
            Engine::Vs(vs) => vs.result(),
            Engine::Single(_) => 0,
        }
    }

    pub fn replay(&self) -> &Replay {
        &self.replay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A compact, comparable fingerprint of a game's visible state.
    fn snapshot(g: &Game) -> (Vec<i32>, i64, i64, i64, i32, i32, i32) {
        let b = g.board();
        let mut grid = Vec::with_capacity((b.width * b.height) as usize);
        for y in 0..b.height {
            for x in 0..b.width {
                grid.push(b.get(x, y).map(|c| c.id()).unwrap_or(-1));
            }
        }
        let s = g.score();
        let (px, py, po) = g
            .current_piece()
            .map(|p| (p.x, p.y, p.orientation))
            .unwrap_or((-99, -99, -99));
        (grid, s.score, s.lines, s.funds, px, py, po)
    }

    const DT: i32 = 16;

    /// Drive a fresh practice game with a scripted input sequence while
    /// recording, then replay the serialized recording and assert the final
    /// state is bit-identical. This is the core determinism guarantee.
    #[test]
    fn practice_replay_is_bit_exact() {
        let seed = 0xC0FFEE;
        let script: Vec<(u32, Input)> = vec![
            (2, Input::MoveLeft),
            (2, Input::MoveLeft),
            (5, Input::Rotate),
            (9, Input::MoveRight),
            (12, Input::BeginDrop),
            (40, Input::Rotate),
            (44, Input::MoveLeft),
            (60, Input::BeginDrop),
            (90, Input::MoveRight),
            (90, Input::MoveRight),
            (120, Input::BeginDrop),
        ];
        let total = 300u32;

        // Live play, recording as we go.
        let mut g = Game::new(seed as u64);
        let mut rec = Recorder::new(seed, Mode::Practice, None, DT, "test");
        let mut si = 0;
        for t in 0..total {
            while si < script.len() && script[si].0 == t {
                let inp = script[si].1.clone();
                inp.apply_to_game(&mut g);
                rec.record(inp);
                si += 1;
            }
            g.tick(DT);
            rec.on_tick();
        }
        let live = snapshot(&g);

        // Round-trip through JSON, then replay.
        let json = rec.to_json();
        let replay: Replay = serde_json::from_str(&json).expect("replay deserializes");
        assert_eq!(replay.tick_count, total);
        let mut player = ReplayPlayer::new(replay);
        player.run_to_end();

        assert_eq!(snapshot(player.player()), live, "replay must reproduce the game exactly");
    }

    /// Same guarantee for a vs-computer match: human inputs are recorded, Ernie
    /// is regenerated from the seed, and both boards must match after replay.
    #[test]
    fn vs_computer_replay_is_bit_exact() {
        let seed = 12345u32;
        let level = 9u32;
        let script: Vec<(u32, Input)> = vec![
            (3, Input::MoveLeft),
            (8, Input::Rotate),
            (15, Input::MoveRight),
            (15, Input::MoveRight),
            (30, Input::BeginDrop),
            (70, Input::Rotate),
            (110, Input::MoveLeft),
            (150, Input::BeginDrop),
        ];
        let total = 1000u32;

        let mut vs = VsComputer::new(seed as u64, level as usize);
        let mut rec = Recorder::new(seed, Mode::VsComputer, Some(level), DT, "test");
        let mut si = 0;
        for t in 0..total {
            while si < script.len() && script[si].0 == t {
                let inp = script[si].1.clone();
                inp.apply_to_game(vs.player_mut());
                rec.record(inp);
                si += 1;
            }
            vs.tick(DT);
            let _ = vs.drain_events();
            rec.on_tick();
        }
        let live_player = snapshot(vs.player());
        let live_ai = snapshot(vs.ai());
        let live_result = vs.result();

        let json = rec.to_json();
        let replay: Replay = serde_json::from_str(&json).expect("replay deserializes");
        let mut player = ReplayPlayer::new(replay);
        player.run_to_end();

        assert_eq!(snapshot(player.player()), live_player, "human board must match");
        assert_eq!(snapshot(player.ai().unwrap()), live_ai, "Ernie's board must match");
        assert_eq!(player.result(), live_result, "match result must match");
    }
}
