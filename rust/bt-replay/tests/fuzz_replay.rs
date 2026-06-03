//! Fuzz → replay bridge.
//!
//! `bt-ai/tests/weapons_fuzz.rs` drives Ernie while injecting random weapons and
//! asserts invariants — but a failing seed was only reproducible as a number,
//! not something you could *watch*. This records the same run (the AI's exact
//! moves + the weapon injections) into the replay format, so any fuzz seed
//! becomes a faithful, scrubbable replay you can open in the library.
//!
//! The catch the format had: the AI hard-drops with `ai_begin_drop` (flat 14),
//! not the human `begin_drop` (28). `Input::AiDrop` closes that, so the
//! recording replays bit-for-bit. `record_fuzz` mirrors the fuzz driver
//! (same seed mixing, same 8% injection rate, same one-move-per-piece gate), so
//! a seed that trips the fuzz reproduces here.

use bt_core::game::GameEvent;
use bt_core::rng::Rng;
use bt_core::weapons::WeaponToken;
use bt_core::{Board, Game};
use bt_replay::{Input, Mode, Recorder, Replay, ReplayPlayer};

fn grid(b: &Board) -> Vec<Vec<Option<i32>>> {
    (0..b.height)
        .map(|y| (0..b.width).map(|x| b.get(x, y).map(|c| c.id())).collect())
        .collect()
}

/// Steer Ernie's current piece to `best_placement`, recording every input
/// (mirrors `Computer::take_turn`, but with a recorder tap on each move).
fn steer_and_record(g: &mut Game, rec: &mut Recorder) {
    let piece = match g.current_piece() {
        Some(p) => p.clone(),
        None => return,
    };
    let placement = bt_ai::best_placement(g.board(), &piece);

    let orientations = piece.orientations.max(1);
    for _ in 0..orientations {
        match g.current_piece() {
            Some(p) if p.orientation == placement.orientation => break,
            None => return,
            _ => {}
        }
        g.rotate();
        rec.record(Input::Rotate);
    }

    let max_moves = g.board().width * 2;
    for _ in 0..max_moves {
        let cur_x = match g.current_piece() {
            Some(p) => p.x,
            None => return,
        };
        if cur_x == placement.x {
            break;
        }
        if cur_x < placement.x {
            g.move_right();
            rec.record(Input::MoveRight);
        } else {
            g.move_left();
            rec.record(Input::MoveLeft);
        }
    }

    g.ai_begin_drop();
    rec.record(Input::AiDrop);
}

/// Drive a recordable fuzz run; return the replay and the live final board.
fn record_fuzz(seed: u32, frames: usize) -> (Replay, Vec<Vec<Option<i32>>>) {
    let mut g = Game::new(seed as u64);
    let mut rec = Recorder::new(seed, Mode::Practice, None, 16, "fuzz-repro");
    let mut rng = Rng::new(seed as u64 ^ 0xF00D_BABE);
    let mut committed = false;

    for _ in 0..frames {
        if rng.rand_below(100) < 8 {
            let tok = WeaponToken::ALL[rng.rand_below(WeaponToken::ALL.len() as i32) as usize];
            g.receive_weapon(tok);
            rec.record(Input::ReceiveWeapon(tok.index() as i32));
        }
        if g.is_in_bazaar() {
            g.leave_bazaar();
            rec.record(Input::LeaveBazaar);
        }
        if !committed && g.current_piece().is_some() {
            steer_and_record(&mut g, &mut rec);
            committed = true;
        }
        g.tick(16);
        rec.on_tick();
        if g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
            committed = false;
        }
        if g.is_game_over() {
            break;
        }
    }

    (rec.to_replay(), grid(g.board()))
}

#[test]
fn fuzz_run_emits_a_faithful_watchable_replay() {
    for &seed in &[2024u32, 7, 1_000_003] {
        let (replay, live) = record_fuzz(seed, 3_000);

        // It's a real recording: AI drops and injected weapons are present.
        assert!(
            replay.frames.iter().any(|f| f.input == Input::AiDrop),
            "seed {seed}: expected AI drops"
        );
        assert!(
            replay.frames.iter().any(|f| matches!(f.input, Input::ReceiveWeapon(_))),
            "seed {seed}: expected weapon injections"
        );

        // Replaying it reproduces the fuzz board bit-for-bit (this is the whole
        // point — a watchable, faithful repro).
        let mut player = ReplayPlayer::new(replay.clone());
        player.run_to_end();
        assert_eq!(
            grid(player.player().board()),
            live,
            "seed {seed}: replay diverged from the live fuzz run"
        );

        // And it survives the library's JSON storage format.
        assert_eq!(Replay::from_json(&replay.to_json()).unwrap(), replay, "seed {seed}: JSON");
    }
}
