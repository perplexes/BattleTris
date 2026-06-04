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
        1 => Just(Op::SoftDrop),
        1 => (0_i32..MAX_WEAPONS).prop_map(Op::ReceiveWeapon),
    ]
}

fn apply_op(g: &mut Game, rec: &mut Recorder, o: &Op) {
    // Drive the LIVE game through DIRECT Game methods — NOT Input::apply_to_game.
    // The replay reconstructs via Input::apply_to_game, so routing the live
    // oracle around that mapping makes the record→replay equality independent of
    // it: a mutant that swaps MoveLeft/MoveRight (or any Input→Game miswiring)
    // inside apply_to_game now diverges the replay from this live run, instead of
    // both sides making the identical mistake and cancelling out.
    match o {
        Op::Tick => {
            g.tick(DT);
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
    ) {
        let seed32 = seed as u32;
        let mut g = Game::new(seed);
        let mut rec = Recorder::new(seed32, Mode::Practice, None, DT, "pbt");

        for o in &ops {
            if g.is_game_over() { break; }
            apply_op(&mut g, &mut rec, o);
        }
        // Always end on a tick boundary so tick_count matches the player.
        if !g.is_game_over() {
            g.tick(DT);
            rec.on_tick();
        }
        let live = fingerprint(&g);

        let replay = rec.to_replay();
        let mut player = ReplayPlayer::new(replay);
        player.run_to_end();

        prop_assert_eq!(
            fingerprint(player.player()),
            live,
            "replay diverged from the live recording"
        );
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

    // ── (c) Replay JSON round-trip ──────────────────────────────────────────
    #[test]
    fn replay_json_round_trip(replay in random_replay(200, 64)) {
        let json = replay.to_json();
        let parsed = Replay::from_json(&json).expect("from_json must succeed");
        prop_assert_eq!(parsed, replay);
    }

    // ── (c′) VersusReplay JSON round-trip ───────────────────────────────────
    #[test]
    fn versus_replay_json_round_trip(replay in random_versus_replay(200, 64)) {
        let json = replay.to_json();
        let parsed = VersusReplay::from_json(&json).expect("from_json must succeed");
        prop_assert_eq!(parsed, replay);
    }
}
