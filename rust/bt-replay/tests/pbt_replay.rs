//! Property-based tests for bt-replay.
//!
//! Mirrors the style of bt-core/tests/pbt.rs (proptest, 128 cases, ~256-op
//! sequences).  Three properties:
//!
//!   (a) RECORD → REPLAY bit-identity  — a Recorder-driven practice game and
//!       the resulting ReplayPlayer produce the **same** render_ids() at the
//!       final tick.
//!
//!   (b) seek(n) == n × step()  — seeking to an arbitrary tick yields the
//!       same board as stepping there one tick at a time.
//!
//!   (c) JSON round-trip — for both Replay and VersusReplay, serialising and
//!       deserialising returns an equal value.

use bt_core::Game;
use bt_replay::{
    Frame, Input, Mode, REPLAY_VERSION, Recorder, Replay, ReplayPlayer, VersusFrame,
    VersusReplay, VersusReplayPlayer,
};
use proptest::prelude::*;

// ─── helpers ────────────────────────────────────────────────────────────────

/// Compact state fingerprint: every cell id, the falling piece position +
/// orientation, AND the score triple (score / lines / funds). Including the
/// score means a replay that reproduces the board but diverges on scoring or
/// funds is caught — the render-only fingerprint missed that whole dimension.
fn fingerprint(g: &Game) -> (Vec<i32>, i32, i32, i32, i64, i64, i64) {
    let ids = g.render_ids();
    let (px, py, po) = g
        .current_piece()
        .map(|p| (p.x, p.y, p.orientation))
        .unwrap_or((-99, -99, -99));
    let s = g.score();
    (ids, px, py, po, s.score, s.lines, s.funds)
}

const DT: i32 = 16;
const MAX_WEAPONS: i32 = bt_core::weapons::BT_MAX_WEAPONS as i32;

// ─── strategies ─────────────────────────────────────────────────────────────

/// A single input that can be applied to a lone Game (no relay inputs).
#[derive(Debug, Clone)]
enum Op {
    MoveLeft,
    MoveRight,
    Rotate,
    BeginDrop,
    AiDrop,
    SoftDrop,
    ReceiveWeapon(i32),
    Tick,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => Just(Op::Tick),
        1 => Just(Op::MoveLeft),
        1 => Just(Op::MoveRight),
        1 => Just(Op::Rotate),
        1 => Just(Op::BeginDrop),
        1 => Just(Op::AiDrop),
        1 => Just(Op::SoftDrop),
        1 => (0_i32..MAX_WEAPONS).prop_map(Op::ReceiveWeapon),
    ]
}

fn apply_op(g: &mut Game, rec: &mut Recorder, o: &Op, dt: i32) {
    // Drive the LIVE game through DIRECT Game methods — NOT Input::apply_to_game.
    // The replay reconstructs via Input::apply_to_game, so routing the live
    // oracle around that mapping makes the record→replay equality independent of
    // it: a mutant that swaps MoveLeft/MoveRight (or any Input→Game miswiring)
    // inside apply_to_game now diverges the replay from this live run, instead of
    // both sides making the identical mistake and cancelling out.
    match o {
        Op::Tick => {
            g.tick(dt);
            rec.on_tick();
        }
        Op::MoveLeft => {
            g.move_left();
            rec.record(Input::MoveLeft);
        }
        Op::MoveRight => {
            g.move_right();
            rec.record(Input::MoveRight);
        }
        Op::Rotate => {
            g.rotate();
            rec.record(Input::Rotate);
        }
        Op::BeginDrop => {
            g.begin_drop();
            rec.record(Input::BeginDrop);
        }
        Op::AiDrop => {
            g.ai_begin_drop();
            rec.record(Input::AiDrop);
        }
        Op::SoftDrop => {
            g.soft_drop();
            rec.record(Input::SoftDrop);
        }
        Op::ReceiveWeapon(t) => {
            if let Some(tok) = bt_core::WeaponToken::from_index(*t) {
                g.receive_weapon(tok);
            }
            rec.record(Input::ReceiveWeapon(*t));
        }
    }
}

/// A vs-computer-legal player input (the human side only — Ernie is regenerated
/// from the seed, never recorded). Excludes weapon/score injections that aren't
/// part of a normal vs-computer recording.
fn vs_computer_input() -> impl Strategy<Value = Input> {
    prop_oneof![
        Just(Input::MoveLeft),
        Just(Input::MoveRight),
        Just(Input::Rotate),
        Just(Input::BeginDrop),
        Just(Input::SoftDrop),
    ]
}

/// Random input for a Versus replay frame (two sides, no relay events needed
/// for the VersusReplay path — side-crossing relay happens inside Versus).
fn versus_input() -> impl Strategy<Value = Input> {
    prop_oneof![
        Just(Input::MoveLeft),
        Just(Input::MoveRight),
        Just(Input::Rotate),
        Just(Input::BeginDrop),
        Just(Input::SoftDrop),
    ]
}

fn versus_frame(max_tick: u32) -> impl Strategy<Value = VersusFrame> {
    (0_u32..max_tick, 0_u8..2_u8, versus_input()).prop_map(|(tick, side, input)| VersusFrame {
        tick,
        side,
        input,
    })
}

fn random_replay(max_ticks: u32, max_frames: usize) -> impl Strategy<Value = Replay> {
    let frames_strategy = prop::collection::vec(
        (0_u32..max_ticks, versus_input()),
        0..max_frames,
    )
    .prop_map(|mut pairs| {
        // frames must be sorted by tick for the player cursor to work
        pairs.sort_by_key(|(t, _)| *t);
        pairs
            .into_iter()
            .map(|(tick, input)| Frame { tick, input })
            .collect::<Vec<_>>()
    });

    (any::<u32>(), frames_strategy).prop_map(move |(seed, frames)| Replay {
        version: REPLAY_VERSION,
        seed,
        mode: Mode::Practice,
        ai_level: None,
        dt_ms: DT,
        engine_sha: "pbt".to_string(),
        tick_count: max_ticks,
        frames,
        title: None,
    })
}

fn random_versus_replay(max_ticks: u32, max_frames: usize) -> impl Strategy<Value = VersusReplay> {
    let frames_strategy =
        prop::collection::vec(versus_frame(max_ticks), 0..max_frames).prop_map(|mut v| {
            v.sort_by_key(|f| f.tick);
            v
        });

    (any::<u32>(), any::<u32>(), frames_strategy).prop_map(
        move |(seed_a, seed_b, frames)| VersusReplay {
            version: REPLAY_VERSION,
            seed_a,
            seed_b,
            dt_ms: DT,
            engine_sha: "pbt".to_string(),
            tick_count: max_ticks,
            frames,
            title: None,
        },
    )
}

/// EVERY `Input` variant, including the relay-internal and parameterised ones —
/// for the JSON round-trip, which must preserve the full enum (not just the 5
/// movement inputs the executable strategies use).
fn any_input() -> impl Strategy<Value = Input> {
    prop_oneof![
        Just(Input::MoveLeft),
        Just(Input::MoveRight),
        Just(Input::Rotate),
        Just(Input::BeginDrop),
        Just(Input::AiDrop),
        Just(Input::SoftDrop),
        any::<u32>().prop_map(Input::LaunchWeapon),
        any::<i32>().prop_map(Input::BuyWeapon),
        any::<i32>().prop_map(Input::SellWeapon),
        Just(Input::LeaveBazaar),
        any::<bool>().prop_map(Input::SetPaused),
        any::<i32>().prop_map(Input::ReceiveWeapon),
        (any::<i64>(), any::<i64>(), any::<i64>())
            .prop_map(|(score, lines, funds)| Input::ReceiveOpScore { score, lines, funds }),
        any::<i64>().prop_map(Input::AddFunds),
    ]
}

/// A `Replay` with full field variety for the JSON round-trip ONLY (it isn't
/// executed): every `Mode`, an optional `ai_level`/`title`, and the full `Input`
/// enum. Catches a serde mutant that drops/renames `title` or `ai_level`, or that
/// silently substitutes an input variant on (de)serialise.
fn json_replay() -> impl Strategy<Value = Replay> {
    let mode = prop_oneof![
        Just(Mode::Practice),
        Just(Mode::VsComputer),
        Just(Mode::VsPlayer),
    ];
    let frames = prop::collection::vec((0_u32..1000, any_input()), 0..48)
        .prop_map(|mut v| { v.sort_by_key(|(t, _)| *t);
            v.into_iter().map(|(tick, input)| Frame { tick, input }).collect::<Vec<_>>() });
    (
        any::<u32>(), mode, proptest::option::of(any::<u32>()), any::<u32>(),
        proptest::option::of("[a-zA-Z0-9 _-]{0,40}"), frames,
    ).prop_map(|(seed, mode, ai_level, tick_count, title, frames)| Replay {
        version: REPLAY_VERSION,
        seed, mode, ai_level, dt_ms: DT, engine_sha: "pbt-json".to_string(),
        tick_count, frames, title,
    })
}

/// A `VersusReplay` with full variety for the JSON round-trip ONLY.
fn json_versus_replay() -> impl Strategy<Value = VersusReplay> {
    let frames = prop::collection::vec((0_u32..1000, 0_u8..2, any_input()), 0..48)
        .prop_map(|mut v| { v.sort_by_key(|(t, _, _)| *t);
            v.into_iter().map(|(tick, side, input)| VersusFrame { tick, side, input }).collect::<Vec<_>>() });
    (
        any::<u32>(), any::<u32>(), any::<u32>(),
        proptest::option::of("[a-zA-Z0-9 _-]{0,40}"), frames,
    ).prop_map(|(seed_a, seed_b, tick_count, title, frames)| VersusReplay {
        version: REPLAY_VERSION,
        seed_a, seed_b, dt_ms: DT, engine_sha: "pbt-json".to_string(),
        tick_count, frames, title,
    })
}

// ─── properties ─────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    // ── (a) RECORD → REPLAY bit-identity ────────────────────────────────────
    //
    // Drive a fresh Game while recording every input; then give the resulting
    // Replay to a ReplayPlayer and assert that the final render fingerprint
    // matches what the live game produced.
    #[test]
    fn record_replay_bit_identity(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
        // A VARIABLE timestep (often NOT 16). The recorded `dt_ms` drives WHEN
        // gravity fires, so a `ReplayPlayer::step` that ticks with a literal `16`
        // instead of `self.replay.dt_ms` diverges from a live run recorded at a
        // different dt. The old hardcoded DT=16 couldn't tell them apart.
        dt in 8i32..40,
        // A non-empty engine SHA so a `to_replay` that drops/blanks it is caught.
        sha in "[a-f0-9]{7,40}",
    ) {
        let seed32 = seed as u32;
        let mut g = Game::new(seed);
        let mut rec = Recorder::new(seed32, Mode::Practice, None, dt, &sha);

        for o in &ops {
            if g.is_game_over() { break; }
            apply_op(&mut g, &mut rec, o, dt);
        }
        // Always end on a tick boundary so tick_count matches the player.
        if !g.is_game_over() {
            g.tick(dt);
            rec.on_tick();
        }
        let live = fingerprint(&g);

        let replay = rec.to_replay();
        // The exported HEADER must carry the recorder's metadata, not defaults —
        // a `to_replay` with `version: 0` / `engine_sha: String::new()` /
        // `mode/seed` drift is caught here.
        prop_assert_eq!(replay.version, REPLAY_VERSION, "to_replay must stamp the current REPLAY_VERSION");
        prop_assert_eq!(replay.dt_ms, dt, "to_replay must preserve dt_ms");
        prop_assert_eq!(&replay.engine_sha, &sha, "to_replay must preserve the engine SHA");
        prop_assert_eq!(replay.mode, Mode::Practice, "to_replay must preserve the mode");
        prop_assert_eq!(replay.seed, seed32, "to_replay must preserve the seed");
        prop_assert_eq!(replay.ai_level, None, "Practice mode has no ai_level");

        let mut player = ReplayPlayer::new(replay);
        player.run_to_end();

        prop_assert_eq!(
            fingerprint(player.player()),
            live,
            "replay diverged from the live recording (dt={})", dt
        );
    }

    // ── (a′) RECORD → REPLAY for a VS-COMPUTER match ────────────────────────
    //
    // `random_replay` only ever builds `Mode::Practice` replays, so the whole
    // VsComputer reconstruction path (Ernie regenerated from the seed, the relay
    // re-run) was UNTESTED by the property suite — a mutant in
    // `ReplayPlayer::new`'s `Mode::VsComputer` arm, or in how `step` ticks the Vs
    // engine, would not be caught. Here we record a real vs-computer match with
    // random HUMAN inputs, then replay it in VsComputer mode and assert BOTH
    // boards (human + Ernie) AND the match result reconstruct exactly.
    #[test]
    fn vs_computer_record_replay_bit_identity(
        seed in any::<u32>(),
        // Ernie difficulty index into bt_ai::AI_LEVELS (0 = Comatose .. 14 = Bionic).
        level in 0u32..(bt_ai::AI_LEVELS.len() as u32),
        // (tick, input) pairs, sorted, applied to the human side during the run.
        script in prop::collection::vec((0u32..600u32, vs_computer_input()), 0..64),
    ) {
        use bt_ai::VsComputer;

        let total = 600u32;
        let mut sorted = script.clone();
        sorted.sort_by_key(|(t, _)| *t);

        let mut vs = VsComputer::new(seed as u64, level as usize);
        let mut rec = Recorder::new(seed, Mode::VsComputer, Some(level), DT, "pbt");
        let mut si = 0usize;
        for t in 0..total {
            // Apply every input stamped at tick t to the HUMAN side, recording it.
            while si < sorted.len() && sorted[si].0 == t {
                let inp = sorted[si].1.clone();
                inp.apply_to_game(vs.player_mut());
                rec.record(inp);
                si += 1;
            }
            vs.tick(DT);
            let _ = vs.drain_events();
            rec.on_tick();
        }
        let live_player = fingerprint(vs.player());
        let live_ai = fingerprint(vs.ai());
        let live_result = vs.result();

        // Replay it (VsComputer mode -> Ernie regenerated, relay re-run).
        let replay = rec.to_replay();
        prop_assert_eq!(replay.mode, Mode::VsComputer, "recorded mode must be VsComputer");
        let mut player = ReplayPlayer::new(replay);
        player.run_to_end();

        prop_assert_eq!(fingerprint(player.player()), live_player,
            "vs-computer replay diverged on the HUMAN board");
        let replay_ai = player.ai().expect("VsComputer replay must expose Ernie's board");
        prop_assert_eq!(fingerprint(replay_ai), live_ai,
            "vs-computer replay diverged on ERNIE's board");
        prop_assert_eq!(player.result(), live_result,
            "vs-computer replay diverged on the match result");
    }

    // ── (b‴) BACKWARD seek on a VS-COMPUTER replay rebuilds in VsComputer mode ─
    //
    // `ReplayPlayer::seek` rebuilds from `self.replay.clone()` for a backward seek.
    // The single-player seek tests only use Practice replays, so a rebuild that
    // forced `Mode::Practice` (dropping the VsComputer engine + Ernie) would slip
    // through. Here we record a real VsComputer match, seek forward to `hi` then
    // BACK to `lo`, and compare BOTH the human AND Ernie boards to a fresh player
    // stepped straight to `lo` — the rebuild MUST stay VsComputer.
    #[test]
    fn vs_computer_backward_seek_rebuilds_in_mode(
        seed in any::<u32>(),
        level in 0u32..(bt_ai::AI_LEVELS.len() as u32),
        script in prop::collection::vec((0u32..300u32, vs_computer_input()), 0..32),
        x in 0u32..300,
        y in 0u32..300,
    ) {
        use bt_ai::VsComputer;
        let total = 300u32;
        let mut sorted = script.clone();
        sorted.sort_by_key(|(t, _)| *t);

        // Record a VsComputer match.
        let mut vs = VsComputer::new(seed as u64, level as usize);
        let mut rec = Recorder::new(seed, Mode::VsComputer, Some(level), DT, "pbt");
        let mut si = 0usize;
        for t in 0..total {
            while si < sorted.len() && sorted[si].0 == t {
                sorted[si].1.clone().apply_to_game(vs.player_mut());
                rec.record(sorted[si].1.clone());
                si += 1;
            }
            vs.tick(DT);
            let _ = vs.drain_events();
            rec.on_tick();
        }
        let replay = rec.to_replay();

        let hi = x.max(y).min(replay.tick_count);
        let lo = x.min(y).min(replay.tick_count);

        // Forward to hi, then BACK to lo (the rebuild path).
        let mut p = ReplayPlayer::new(replay.clone());
        p.seek(hi);
        p.seek(lo);

        // Fresh player stepped straight to lo.
        let mut q = ReplayPlayer::new(replay.clone());
        for _ in 0..lo { if !q.step() { break; } }

        prop_assert_eq!(p.tick_index(), q.tick_index(),
            "vs-computer backward-seek tick_index mismatch (hi={}, lo={})", hi, lo);
        prop_assert_eq!(fingerprint(p.player()), fingerprint(q.player()),
            "vs-computer backward-seek HUMAN board mismatch");
        // The rebuild must stay VsComputer (Ernie present), and his board must match.
        let p_ai = p.ai().expect("backward-seek must keep the VsComputer engine (Ernie present)");
        let q_ai = q.ai().expect("stepped VsComputer player must have Ernie");
        prop_assert_eq!(fingerprint(p_ai), fingerprint(q_ai),
            "vs-computer backward-seek ERNIE board mismatch (hi={}, lo={})", hi, lo);
        prop_assert_eq!(p.result(), q.result(), "vs-computer backward-seek result mismatch");
    }

    // ── (b) seek(n) == n × step() ───────────────────────────────────────────
    //
    // Build a replay with a random sequence of (sorted) frames, pick a random
    // target tick ≤ tick_count, then assert that seek() and repeated step()
    // land in the same state.
    #[test]
    fn seek_equals_step(
        replay in random_replay(200, 64),
        target in 0_u32..200_u32,
    ) {
        let target = target.min(replay.tick_count);

        // Ground truth: step exactly `target` times from fresh.
        let mut stepped = ReplayPlayer::new(replay.clone());
        for _ in 0..target {
            if !stepped.step() { break; }
        }

        // Under test: seek to the same target.
        let mut sought = ReplayPlayer::new(replay.clone());
        sought.seek(target);

        prop_assert_eq!(
            sought.tick_index(),
            stepped.tick_index(),
            "tick_index diverged after seek vs step"
        );
        prop_assert_eq!(
            fingerprint(sought.player()),
            fingerprint(stepped.player()),
            "board diverged after seek vs step"
        );
    }

    // ── (b″) BACKWARD seek rebuilds correctly ───────────────────────────────
    //
    // Forward seek is just stepping; a BACKWARD seek (target < current) takes
    // seek's distinct rebuild-from-the-seeds path. Seek forward to `hi`, back to
    // `lo`, and compare to a fresh player stepped to `lo`.
    #[test]
    fn seek_backward_rebuilds_correctly(
        replay in random_replay(200, 64),
        x in 0_u32..200_u32,
        y in 0_u32..200_u32,
    ) {
        let hi = x.max(y).min(replay.tick_count);
        let lo = x.min(y).min(replay.tick_count);

        let mut p = ReplayPlayer::new(replay.clone());
        p.seek(hi);
        p.seek(lo); // backward (unless hi == lo) -> rebuild path

        let mut q = ReplayPlayer::new(replay.clone());
        for _ in 0..lo { if !q.step() { break; } }

        prop_assert_eq!(p.tick_index(), q.tick_index(), "backward-seek tick_index mismatch");
        prop_assert_eq!(fingerprint(p.player()), fingerprint(q.player()),
            "backward-seek state mismatch (hi={}, lo={})", hi, lo);
    }

    // ── (b′) VersusReplayPlayer seek(n) == n × step() ───────────────────────
    #[test]
    fn versus_seek_equals_step(
        replay in random_versus_replay(200, 64),
        target in 0_u32..200_u32,
    ) {
        let target = target.min(replay.tick_count);

        let mut stepped = VersusReplayPlayer::new(replay.clone());
        for _ in 0..target {
            if !stepped.step() { break; }
        }

        let mut sought = VersusReplayPlayer::new(replay.clone());
        sought.seek(target);

        prop_assert_eq!(
            sought.tick_index(),
            stepped.tick_index(),
            "VersusReplayPlayer tick_index diverged"
        );
        prop_assert_eq!(
            fingerprint(sought.game(true)),
            fingerprint(stepped.game(true)),
            "side-A board diverged after seek vs step"
        );
        prop_assert_eq!(
            fingerprint(sought.game(false)),
            fingerprint(stepped.game(false)),
            "side-B board diverged after seek vs step"
        );
    }

    // ── (b‴) VersusReplayPlayer BACKWARD seek rebuilds correctly ─────────────
    //
    // `versus_seek_equals_step` only seeks FORWARD from a fresh player, so the
    // distinct rebuild-from-the-seeds branch in `VersusReplayPlayer::seek`
    // (taken only when `target < executed`) was untested — a mutant
    // `if false && target < self.executed { ... }` (never rebuild) survived. Seek
    // forward to `hi`, then BACK to `lo`, and compare both sides to a fresh player
    // stepped straight to `lo`.
    #[test]
    fn versus_seek_backward_rebuilds_correctly(
        replay in random_versus_replay(200, 64),
        x in 0_u32..200_u32,
        y in 0_u32..200_u32,
    ) {
        let hi = x.max(y).min(replay.tick_count);
        let lo = x.min(y).min(replay.tick_count);

        let mut p = VersusReplayPlayer::new(replay.clone());
        p.seek(hi);
        p.seek(lo); // backward (unless hi == lo) -> the rebuild path

        let mut q = VersusReplayPlayer::new(replay.clone());
        for _ in 0..lo { if !q.step() { break; } }

        prop_assert_eq!(p.tick_index(), q.tick_index(),
            "VersusReplayPlayer backward-seek tick_index mismatch (hi={}, lo={})", hi, lo);
        prop_assert_eq!(fingerprint(p.game(true)), fingerprint(q.game(true)),
            "side-A diverged after backward seek (hi={}, lo={})", hi, lo);
        prop_assert_eq!(fingerprint(p.game(false)), fingerprint(q.game(false)),
            "side-B diverged after backward seek (hi={}, lo={})", hi, lo);
    }

    // ── (b⁗) seek BEYOND tick_count clamps to the end ───────────────────────
    //
    // `seek` clamps `target` to `tick_count`. Seeking to a tick PAST the end must
    // land exactly at the end — same `tick_index` and state as `run_to_end()`.
    // The existing seek tests only use `target <= tick_count`, so the clamp was
    // unexercised. Covers both the single-player and versus players.
    #[test]
    fn seek_past_end_equals_run_to_end(
        replay in random_replay(200, 64),
        vreplay in random_versus_replay(200, 64),
        overshoot in 1_u32..1_000_000,
    ) {
        // Single-player.
        let mut sought = ReplayPlayer::new(replay.clone());
        sought.seek(replay.tick_count.saturating_add(overshoot));
        let mut ran = ReplayPlayer::new(replay.clone());
        ran.run_to_end();
        prop_assert_eq!(sought.tick_index(), ran.tick_index(),
            "seek-past-end tick_index must clamp to run_to_end (single)");
        prop_assert_eq!(sought.tick_index(), replay.tick_count,
            "seek-past-end must land exactly at tick_count (single)");
        prop_assert_eq!(fingerprint(sought.player()), fingerprint(ran.player()),
            "seek-past-end state must equal run_to_end (single)");

        // Versus.
        let mut vsought = VersusReplayPlayer::new(vreplay.clone());
        vsought.seek(vreplay.tick_count.saturating_add(overshoot));
        let mut vran = VersusReplayPlayer::new(vreplay.clone());
        vran.run_to_end();
        prop_assert_eq!(vsought.tick_index(), vran.tick_index(),
            "seek-past-end tick_index must clamp to run_to_end (versus)");
        prop_assert_eq!(vsought.tick_index(), vreplay.tick_count,
            "seek-past-end must land exactly at tick_count (versus)");
        prop_assert_eq!(fingerprint(vsought.game(true)), fingerprint(vran.game(true)),
            "seek-past-end side-A must equal run_to_end (versus)");
        prop_assert_eq!(fingerprint(vsought.game(false)), fingerprint(vran.game(false)),
            "seek-past-end side-B must equal run_to_end (versus)");
        prop_assert_eq!(vsought.result(), vran.result(),
            "seek-past-end result must equal run_to_end (versus)");
    }

    // ── (a‴) Input::LaunchWeapon EXECUTION is semantically pinned ────────────
    //
    // `Input::apply_to_game(LaunchWeapon(slot))` must actually launch the weapon
    // in that slot: consume it from the arsenal AND emit a `WeaponLaunched` event
    // for the relay. Replay/server PBTs mostly check accept/ack/recording, so a
    // mutant `Input::LaunchWeapon(_slot) => {}` (a silent no-op) survives them.
    // Here we grant a weapon into a known slot, apply the Input, and assert both
    // the arsenal decrement and the emitted launch event.
    #[test]
    fn launch_weapon_input_consumes_arsenal_and_emits_event(
        seed in any::<u64>(),
        tok in 0_i32..MAX_WEAPONS,
    ) {
        use bt_core::GameEvent;
        let Some(token) = bt_core::WeaponToken::from_index(tok) else { return Ok(()); };
        let mut g = Game::new(seed);
        // Grant the token; find the slot it landed in.
        prop_assume!(g.grant_weapon(token));
        let slot = (0..10usize).find(|&s| g.arsenal_token(s) == tok)
            .expect("granted token must occupy a slot");
        let qty_before = g.arsenal_quantity(slot);
        let _ = g.take_events(); // clear any spawn/start events

        Input::LaunchWeapon(slot as u32).apply_to_game(&mut g);

        // The arsenal slot must have one fewer (consumed).
        let qty_after = g.arsenal_quantity(slot);
        prop_assert_eq!(qty_after, qty_before - 1,
            "LaunchWeapon must consume one from arsenal slot {} ({} -> {})",
            slot, qty_before, qty_after);
        // And a WeaponLaunched(token) event must be queued for the relay.
        let launched = g.take_events().into_iter()
            .any(|e| matches!(e, GameEvent::WeaponLaunched(t) if t == token));
        prop_assert!(launched,
            "LaunchWeapon must emit WeaponLaunched({:?}) for the relay", token);
    }

    // ── (c) Replay JSON round-trip ──────────────────────────────────────────
    //    Uses the FULL-variety `json_replay` (every Mode, optional ai_level/title,
    //    every Input variant) so a serde mutant that drops/renames `title` or
    //    `ai_level`, or substitutes an input on (de)serialise, is caught — the old
    //    strategy hardcoded Practice/None/None and a 5-input subset.
    #[test]
    fn replay_json_round_trip(replay in json_replay()) {
        let json = replay.to_json();
        let parsed = Replay::from_json(&json).expect("from_json must succeed");
        prop_assert_eq!(parsed, replay);
    }

    // ── (c′) VersusReplay JSON round-trip ───────────────────────────────────
    #[test]
    fn versus_replay_json_round_trip(replay in json_versus_replay()) {
        let json = replay.to_json();
        let parsed = VersusReplay::from_json(&json).expect("from_json must succeed");
        prop_assert_eq!(parsed, replay);
    }
}
