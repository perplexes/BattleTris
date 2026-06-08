//! Deterministic recording + playback for BattleTris.
//!
//! The engine is fully deterministic: [`bt_core::Game`] is seeded (drand48) and
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
//! Faithful replay requires a fixed timestep: the host must advance the
//! engine in constant `dt_ms` steps (an accumulator loop) rather than
//! wall-clock frame deltas. [`ReplayPlayer`] does exactly that.
//!
//! ## Recording vs trace fidelity
//!
//! This is a *seed replay*: it records only the inputs and regenerates everything
//! else (gravity, Ernie's moves, RNG) by re-running the engine. It is therefore
//! faithful only on the same engine build. To debug a regression, replay the
//! same inputs on a different `engine_sha` to see where behaviour diverged.
//! `engine_sha` is recorded so a caller can detect a build mismatch.

use bt_ai::VsComputer;
use bt_core::game::GameEvent;
use bt_core::versus::Side;
use bt_core::weapons::WeaponToken;
use bt_core::{Game, Versus};
use serde::{Deserialize, Serialize};

/// The on-disk format version stamped into every recording, so a loader can
/// reject one it can't interpret. Bump it for any incompatible format change.
pub const REPLAY_VERSION: u32 = 1;

/// One state-mutating action the host can apply to a player's [`Game`]. These
/// mirror the input surface of the wasm wrappers (`WasmGame` / `WasmVsComputer`).
///
/// `ReceiveWeapon` / `ReceiveOpScore` / `AddFunds` are *incoming* frames: an
/// effect originating outside this board. In a two-player recording that source
/// is the opponent (an external process whose effects on *this* side must be
/// captured as inputs); a practice recording can also inject a `ReceiveWeapon`
/// to demonstrate a weapon. In vs-computer mode Ernie is replayed from the seed,
/// so his effects are re-simulated rather than recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Input {
    /// Nudge the falling piece one column left.
    MoveLeft,
    /// Nudge the falling piece one column right.
    MoveRight,
    /// Rotate the falling piece one quarter turn.
    Rotate,
    /// The human hard drop: switch the piece to the fast cadence. The depth
    /// bonus `BT_BOARD_HGT - y` is awarded only when this *newly* engages the
    /// drop (a repeat, or a call while paused / in the bazaar / after game over,
    /// scores nothing).
    BeginDrop,
    /// Ernie's flat-scored hard drop (`Game::ai_begin_drop`, BTComputer.C:1255).
    /// Distinct from the human [`BeginDrop`](Input::BeginDrop) (which scores
    /// `BT_BOARD_HGT - y`) so an AI-driven recording (one that captures Ernie's
    /// own moves as inputs) replays bit-for-bit on his scoring curve rather than
    /// the human one.
    AiDrop,
    /// Advance the falling piece down one cell (a blocked piece begins its lock
    /// slide instead). Ignored unless a piece is actively falling (i.e. while
    /// paused, in the bazaar, mid lock-slide, or after game over). One per tap;
    /// the host hold-repeats it.
    SoftDrop,
    /// Fire the weapon in arsenal slot `n` at the opponent.
    LaunchWeapon(u32),
    /// Purchase the weapon with [`WeaponToken`] index `n` at the bazaar.
    BuyWeapon(i32),
    /// Sell the weapon with [`WeaponToken`] index `n` back at the bazaar.
    SellWeapon(i32),
    /// Clear this player's bazaar flag, resuming their local play. (Online, the
    /// shared barrier lifts only once both sides have left; `bt-netcode`'s
    /// predictor forwards this without applying it locally.)
    LeaveBazaar,
    /// Set the paused flag (a recorded input so a pause/unpause reproduces).
    SetPaused(bool),
    /// An incoming weapon ([`WeaponToken`] index `n`, the relay's `BT_WPN_ON`):
    /// queued on this side and applied at the next piece lock. Carried from the
    /// opponent in a two-player recording, or injected directly to demonstrate a
    /// weapon in a showcase / fuzz recording (see the enum doc).
    ReceiveWeapon(i32),
    /// The opponent's score mirror arriving from the relay (`BT_OP_SCORE`), which
    /// drives this side's bazaar countdown and Lawyers' Delite. Two-player relay.
    ReceiveOpScore { score: i64, lines: i64, funds: i64 },
    /// Funds credited to this player. Normally relay-produced for two-player
    /// Mondale/Keating, banking the amount the opponent's `FundsStolen` reported.
    AddFunds(i64),
}

impl Input {
    /// Apply this input to a player's game, discarding the engine methods'
    /// accept/no-op return values.
    ///
    /// The canonical decode from a wire `Input` to the engine: replay playback
    /// routes every input through here, so a recorded input is interpreted one
    /// way wherever it is re-applied. (The netcode predictor reuses this for the
    /// inputs it replays straight, but gates Buy/Sell/LeaveBazaar itself.)
    /// Buy/Sell/Receive turn an out-of-range token index into a silent no-op
    /// rather than panicking, so a stray token in a recording can't crash playback.
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
            Input::AddFunds(amount) => g.add_funds(*amount),
        }
    }
}

/// Which game a recording came from. Selects which engine [`ReplayPlayer`]
/// rebuilds: a lone [`Game`] for the single-board modes, or a full
/// [`bt_ai::VsComputer`] match so Ernie is re-simulated rather than recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Solo play: one board, no opponent.
    Practice,
    /// A match against Ernie, who is replayed from the seed and `ai_level`, not
    /// recorded.
    VsComputer,
    /// One recorded side of a two-player game, played back as a single board: the
    /// opponent's effects on this side arrive as recorded `Receive*` / `AddFunds`
    /// inputs, since the other player is external. (Playback treats it the same as
    /// [`Practice`](Mode::Practice).)
    VsPlayer,
}

/// One recorded input, stamped with the tick index it was applied at (the number
/// of engine ticks already executed when it landed). The stamp is what anchors
/// replay to a fixed timestep: playback applies the input at the same tick
/// boundary, so it lands on the identical engine state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Engine ticks already executed when this input was applied.
    pub tick: u32,
    /// The action applied at that tick.
    pub input: Input,
}

/// A complete, self-contained recording of one player's game: the seed, the
/// engine parameters (mode, `ai_level`, `dt_ms`), and the timestamped inputs are
/// everything needed to reproduce it. Everything not stored here (gravity, RNG,
/// Ernie's moves) is regenerated by re-running the engine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    /// The format version stamped at record time ([`REPLAY_VERSION`]), so a
    /// loader can compare it and reject a recording it can't interpret.
    pub version: u32,
    /// The seed the engine was constructed with, from which all determinism derives.
    pub seed: u32,
    /// Which engine to rebuild for playback.
    pub mode: Mode,
    /// Ernie's difficulty (vs-computer only).
    pub ai_level: Option<u32>,
    /// The fixed timestep the engine was advanced with. Replay must reuse it
    /// exactly; wall-clock deltas would desync.
    pub dt_ms: i32,
    /// The engine build this was recorded against (`git` short SHA, or "dev").
    /// A seed replay is faithful only on the same build; recording it lets a
    /// caller detect a build mismatch against the running engine.
    pub engine_sha: String,
    /// Total engine ticks elapsed at the end of the recording. This is the
    /// playback length, since the last input may land well before the final tick.
    pub tick_count: u32,
    /// The timestamped inputs. Playback assumes they are in tick order (and
    /// applies same-tick inputs in vector order), the order [`Recorder`] writes
    /// them in.
    pub frames: Vec<Frame>,
    /// Optional human label (e.g. a weapon-showcase name). Defaults to `None`
    /// when the field is absent, so a recording without a title still loads.
    #[serde(default)]
    pub title: Option<String>,
}

impl Replay {
    /// Serialize to JSON (the on-the-wire / on-disk form). Cannot fail for this
    /// plain data, so a failure degrades to an empty string rather than a panic.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Parse from JSON. A shape error surfaces as `Err` rather than being
    /// swallowed; it does not check semantics (version, tick order). The loader
    /// is responsible for those checks.
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
    // The match parameters (seed, mode, ai_level, dt_ms, engine_sha): captured
    // at construction and copied verbatim into the produced [`Replay`], so
    // playback rebuilds the identical engine. See [`Replay`] for their meaning.
    seed: u32,
    mode: Mode,
    ai_level: Option<u32>,
    dt_ms: i32,
    engine_sha: String,
    /// The virtual tick clock, bumped by [`Recorder::on_tick`]. Every recorded
    /// input is stamped with its current value.
    tick: u32,
    /// Inputs captured so far, in the order they were applied.
    frames: Vec<Frame>,
    /// Optional human label for the replay library.
    title: Option<String>,
}

impl Recorder {
    /// Start a recording for a match with the given parameters. The clock starts
    /// at tick 0; the host then mirrors its live game by calling
    /// [`record`](Recorder::record) on each input and [`on_tick`](Recorder::on_tick)
    /// after each engine tick.
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

    /// Snapshot the recording so far into an immutable [`Replay`]. Non-consuming
    /// (clones the frames), so the host can export mid-game without ending the
    /// recording.
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

    /// The recording so far as JSON (`to_replay().to_json()`), the form the
    /// host ships to the replay library / bug report.
    pub fn to_json(&self) -> String {
        self.to_replay().to_json()
    }
}

/// The engine being replayed: a lone game (practice / one PvP side) or a full
/// vs-computer match (so Ernie is regenerated from the seed).
// One Engine per ReplayPlayer (never in a hot array/Vec), so the ~2KB size gap
// between a lone Game and a full VsComputer match is irrelevant; boxing both
// variants would only add allocations + deref noise. clippy::large_enum_variant.
#[allow(clippy::large_enum_variant)]
enum Engine {
    /// Practice / one PvP side: a single board, no opponent to regenerate.
    Single(Game),
    /// A full vs-computer match, so Ernie is re-simulated from the seed instead
    /// of recorded.
    Vs(VsComputer),
}

/// Drives a [`Replay`] over the engine at its recorded fixed timestep. Step it
/// tick-by-tick (for scrubbable playback) or run it to the end (headless).
pub struct ReplayPlayer {
    engine: Engine,
    replay: Replay,
    /// Ticks executed so far. This is the playback position, and the value each
    /// pending frame's `tick` is compared against.
    executed: u32,
    /// Index of the next unapplied frame in `replay.frames`. Frames are in tick
    /// order, so a single forward cursor consumes them without rescanning.
    cursor: usize,
}

impl ReplayPlayer {
    /// Build a player positioned at tick 0, with a fresh engine seeded from the
    /// replay. Nothing is applied until [`step`](ReplayPlayer::step).
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

    /// Route one input to the human side. In a vs-computer replay only the human
    /// player's inputs are recorded; Ernie acts inside [`bt_ai::VsComputer::tick`].
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
    /// from the seed and fast-forwards; this is cheap since the engine runs
    /// thousands of ticks per millisecond. This is what a scrubber/seek bar calls.
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

    /// The player's game (read-only), for rendering / inspection.
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

    /// The recording being played, for reading its metadata (title, mode, …).
    pub fn replay(&self) -> &Replay {
        &self.replay
    }
}

// ─── Online (server-authoritative) match replays ──────────────────────────────

/// One recorded client input in a versus replay: which side launched it, stamped
/// with the tick it was applied at. For a server-produced replay, the order of
/// these frames (by tick, then vector position) defines the playback order; both
/// boards derive from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersusFrame {
    /// Engine ticks already executed when this input was applied.
    pub tick: u32,
    /// Which board the input drives. 0 = side A, 1 = side B.
    pub side: u8,
    /// The action applied to that side at that tick.
    pub input: Input,
}

/// A self-contained recording of an online server-authoritative match: the two
/// seeds plus the totally-ordered client-input stream. Because [`Versus`] is
/// deterministic, replaying means re-running it and applying each input at its
/// tick. The whole relay (weapons, taxes, bazaar) reproduces exactly, so none of
/// those derived cross-player effects need to be recorded. A totally-ordered
/// event log is therefore the entire canonical online replay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersusReplay {
    /// The format version stamped at record time ([`REPLAY_VERSION`]), so a
    /// loader can compare it and reject a recording it can't interpret.
    pub version: u32,
    /// Seed for side A's board.
    pub seed_a: u32,
    /// Seed for side B's board.
    pub seed_b: u32,
    /// The fixed timestep both boards were advanced with, reused exactly on
    /// playback.
    pub dt_ms: i32,
    /// The engine build this was recorded against; recording it lets a caller
    /// detect a build mismatch (on which the reproduction may diverge).
    pub engine_sha: String,
    /// Total engine ticks elapsed at the end of the match.
    pub tick_count: u32,
    /// The interleaved per-side inputs. Playback assumes tick order, with vector
    /// order breaking ties.
    pub frames: Vec<VersusFrame>,
    /// Optional human label. Defaults to `None` when absent.
    #[serde(default)]
    pub title: Option<String>,
}

impl VersusReplay {
    /// Serialize to JSON. Cannot fail for this plain data.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    /// Parse from JSON; a malformed recording surfaces as an error.
    pub fn from_json(s: &str) -> Result<VersusReplay, serde_json::Error> {
        serde_json::from_str(s)
    }
}

/// Drives a [`VersusReplay`] over a [`Versus`] at its recorded timestep, so BOTH
/// boards are reproduced. Step tick-by-tick (scrubbable) or run to the end.
pub struct VersusReplayPlayer {
    /// The two-board match. The relay runs inside its own `tick`, so cross-player
    /// effects reproduce without being recorded.
    versus: Versus,
    replay: VersusReplay,
    /// Ticks executed so far (the playback position).
    executed: u32,
    /// Index of the next unapplied frame; advances monotonically since frames
    /// are tick-ordered.
    cursor: usize,
}

impl VersusReplayPlayer {
    /// Build a player at tick 0 with both boards seeded from the replay.
    pub fn new(replay: VersusReplay) -> VersusReplayPlayer {
        let versus = Versus::new(replay.seed_a as u64, replay.seed_b as u64);
        VersusReplayPlayer { versus, replay, executed: 0, cursor: 0 }
    }

    /// Advance one tick: apply every input stamped at the current tick, then tick
    /// the match. Returns false once the recording is exhausted.
    pub fn step(&mut self) -> bool {
        if self.executed >= self.replay.tick_count {
            return false;
        }
        while self.cursor < self.replay.frames.len()
            && self.replay.frames[self.cursor].tick == self.executed
        {
            let input = self.replay.frames[self.cursor].input.clone();
            let side = if self.replay.frames[self.cursor].side == 0 { Side::A } else { Side::B };
            input.apply_to_game(self.versus.game_mut(side));
            self.cursor += 1;
        }
        self.versus.tick(self.replay.dt_ms);
        // Drain both sides' events so a long replay doesn't accumulate them.
        let _: Vec<GameEvent> = self.versus.game_mut(Side::A).take_events();
        let _: Vec<GameEvent> = self.versus.game_mut(Side::B).take_events();
        self.executed += 1;
        true
    }

    /// Run the whole match to its end (headless reproduction).
    pub fn run_to_end(&mut self) {
        while self.step() {}
    }

    /// Jump to tick `target` (backward seeks rebuild from the seeds + fast-forward).
    pub fn seek(&mut self, target: u32) {
        let target = target.min(self.replay.tick_count);
        if target < self.executed {
            *self = VersusReplayPlayer::new(self.replay.clone());
        }
        while self.executed < target && self.step() {}
    }

    /// Ticks executed so far (the playback position).
    pub fn tick_index(&self) -> u32 {
        self.executed
    }

    /// A side's game (read-only): `true` for A, `false` for B.
    pub fn game(&self, side_a: bool) -> &Game {
        self.versus.game(if side_a { Side::A } else { Side::B })
    }

    /// 0 = ongoing, 1 = A won, 2 = B won.
    pub fn result(&self) -> i32 {
        self.versus.result()
    }

    /// The recording being played, for reading its metadata.
    pub fn replay(&self) -> &VersusReplay {
        &self.replay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versus_replay_round_trips_and_reproduces_deterministically() {
        let frames = vec![
            VersusFrame { tick: 2, side: 0, input: Input::MoveLeft },
            VersusFrame { tick: 3, side: 1, input: Input::MoveRight },
            VersusFrame { tick: 5, side: 0, input: Input::BeginDrop },
            VersusFrame { tick: 20, side: 1, input: Input::BeginDrop },
            VersusFrame { tick: 40, side: 0, input: Input::BeginDrop },
            VersusFrame { tick: 60, side: 1, input: Input::Rotate },
        ];
        let replay = VersusReplay {
            version: REPLAY_VERSION,
            seed_a: 111,
            seed_b: 222,
            dt_ms: 16,
            engine_sha: "test".into(),
            tick_count: 200,
            frames,
            title: None,
        };

        // JSON round-trips exactly.
        let parsed = VersusReplay::from_json(&replay.to_json()).expect("parses");
        assert_eq!(parsed, replay);

        // Two independent plays reproduce BOTH boards + the result identically.
        let fingerprint = |r: &VersusReplay| {
            let mut p = VersusReplayPlayer::new(r.clone());
            p.run_to_end();
            (p.game(true).render_ids(), p.game(false).render_ids(), p.result())
        };
        assert_eq!(fingerprint(&replay), fingerprint(&replay), "deterministic reproduction");

        // Seeking to the end matches running to the end.
        let mut stepped = VersusReplayPlayer::new(replay.clone());
        stepped.run_to_end();
        let mut sought = VersusReplayPlayer::new(replay.clone());
        sought.seek(10_000); // clamps to tick_count
        assert_eq!(sought.tick_index(), replay.tick_count);
        assert_eq!(stepped.game(true).render_ids(), sought.game(true).render_ids());
        assert_eq!(stepped.game(false).render_ids(), sought.game(false).render_ids());
    }

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
