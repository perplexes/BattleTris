//! Headless end-to-end tests for the vs-computer match.
//!
//! These drive [`bt_ai::VsComputer`] over a virtual clock — exactly what the
//! browser's `requestAnimationFrame` loop does, minus the rendering — so the
//! whole match (bazaar barrier, difficulty throttle, score relay, win
//! detection) is exercised deterministically with no browser, no wasm, and no
//! wall-clock waiting.

use bt_ai::{VsComputer, AI_LEVELS};
use bt_core::constants::BT_BOARD_HGT;

const DT: i32 = 16; // ~60fps, the front-end's fixed step

/// Regression: a Comatose (level 0, 4000ms/move) Ernie must NOT score the
/// instant the match starts. The port used to steer + hard-drop the AI's first
/// piece inside `VsComputer::new`, banking the *human* hard-drop bonus
/// (`BT_BOARD_HGT - y` = 28) before the first tick — so a do-nothing Ernie
/// showed 28 points immediately. Faithfully, the original `BTComputer`
/// schedules its first move one `delay_` later (BTComputer.C, `addTimeout`)
/// and banks a flat `BT_BOARD_HGT / 2` (= 14) per piece (BTComputer.C:1255),
/// never the human drop bonus.
#[test]
fn comatose_ernie_does_not_score_immediately() {
    let mut vs = VsComputer::new(12_345, 0); // Comatose: 4000ms / move
    vs.player_mut().set_paused(true); // isolate Ernie

    // Right after construction: nothing placed, zero score.
    assert_eq!(
        vs.ai().score().score,
        0,
        "Ernie must not score before its first move (no constructor placement)"
    );

    // ...and still zero a couple seconds in, well under the 4000ms delay.
    for _ in 0..(2000 / DT as usize) {
        vs.tick(DT);
    }
    assert_eq!(
        vs.ai().score().score,
        0,
        "Comatose Ernie is idle (score 0) until its first throttled move"
    );

    // Run until Ernie finally commits its first piece, then check the award.
    let mut first_score = 0;
    for _ in 0..(6000 / DT as usize) {
        vs.tick(DT);
        if vs.ai().score().score > 0 {
            first_score = vs.ai().score().score;
            break;
        }
    }
    assert_eq!(
        first_score,
        (BT_BOARD_HGT / 2) as i64,
        "Ernie's per-piece score is the flat BT_BOARD_HGT/2, not the 28-pt human hard-drop bonus"
    );
}

/// Advance the AI (with the human paused so it can't top out and end the match
/// early) until the human is pulled into the bazaar. The human enters when the
/// AI clears its 20th line: that score is relayed as `op_score`, and the
/// player's combined-line bazaar trigger fires. Returns the number of ticks it
/// took, or `None` if the cap was hit / the match ended first.
fn run_to_player_bazaar(vs: &mut VsComputer, max_ticks: usize) -> Option<usize> {
    vs.player_mut().set_paused(true);
    for t in 0..max_ticks {
        vs.tick(DT);
        if vs.player().is_in_bazaar() {
            return Some(t);
        }
        if vs.result() != 0 {
            return None;
        }
    }
    None
}

/// The bug report was "the AI score was going up real fast" — the AI kept
/// playing while the human was stuck shopping. Faithfully, the bazaar is a
/// synchronized barrier: `BTGame` pauses ALL timeouts and only resumes once
/// BOTH players have left (BattleTris(1) / `BTComputer::checkBazaar`). This
/// asserts the AI is frozen for as long as the human is in the bazaar, and
/// resumes the instant the human hits DONE.
#[test]
fn bazaar_is_a_synchronized_barrier() {
    let mut vs = VsComputer::new(12345, 9); // level 9 = "Pepped-up" (350ms): functional & quick
    let ticks = run_to_player_bazaar(&mut vs, 200_000)
        .expect("the human should be pulled into the bazaar once the AI clears 20 lines");
    assert!(vs.player().is_in_bazaar());
    eprintln!("reached bazaar after {ticks} ticks; AI lines = {}", vs.ai().score().lines);

    // Snapshot the AI now that the human is shopping.
    let ai_lines = vs.ai().score().lines;
    let ai_score = vs.ai().score().score;
    let op_score = vs.player().score().op_score;
    assert!(ai_lines >= 20, "AI should have cleared >= 20 lines to open the bazaar");

    // While the human is in the bazaar the WHOLE match is frozen. Tick a long
    // time: the AI must not advance one bit (this is the regression).
    for _ in 0..5_000 {
        vs.tick(DT);
    }
    assert!(vs.player().is_in_bazaar(), "human stays in the bazaar until DONE");
    assert_eq!(vs.ai().score().lines, ai_lines, "AI lines must be frozen in the bazaar");
    assert_eq!(vs.ai().score().score, ai_score, "AI score must be frozen in the bazaar");
    assert_eq!(vs.player().score().op_score, op_score, "the op-score mirror is frozen too");

    // Human hits DONE → play resumes → the AI advances again.
    vs.player_mut().leave_bazaar();
    let mut resumed = false;
    for _ in 0..20_000 {
        vs.tick(DT);
        if vs.ai().score().lines > ai_lines {
            resumed = true;
            break;
        }
    }
    assert!(resumed, "the AI should resume placing pieces after the human leaves the bazaar");
}

/// The other half of the bug surfaced in the browser: at the fastest difficulty
/// the AI seemed to clear *no* lines and just top out. Characterize every
/// difficulty headlessly: with the human paused, run a fixed virtual-time
/// budget and record how many lines Ernie clears. This proves Ernie is
/// functional across the whole slider (it places pieces and clears lines at
/// every level — the "0 lines" the browser showed was a measurement artifact,
/// not engine behavior).
#[test]
fn ernie_clears_lines_at_every_difficulty() {
    // ~120s of virtual time — enough for even Comatose (4000ms/move) to place
    // a couple dozen pieces.
    let budget_ticks = 120_000 / DT as usize;

    for level in 0..AI_LEVELS.len() {
        let mut vs = VsComputer::new(2024 + level as u64, level);
        vs.player_mut().set_paused(true); // isolate the AI; no human top-out

        let mut topped_out_at = None;
        for t in 0..budget_ticks {
            vs.tick(DT);
            // The human is paused, so a non-zero result means the AI topped out.
            if vs.result() != 0 {
                topped_out_at = Some(t);
                break;
            }
        }

        let lines = vs.ai().score().lines;
        eprintln!(
            "level {level:2} ({:5}ms): AI cleared {lines:3} lines{}",
            AI_LEVELS[level],
            match topped_out_at {
                Some(t) => format!(" (topped out at tick {t})"),
                None => String::new(),
            }
        );
        assert!(
            lines > 0,
            "Ernie should clear at least one line at level {level} ({}ms)",
            AI_LEVELS[level]
        );
    }
}

/// When the human tops out, the match must end as a loss (`result == 2`) — the
/// signal the front-end uses to show "GAME OVER". We force a fast top-out by
/// jamming every piece to the left wall (so rows never complete) and
/// hard-dropping; the AI is left at Comatose so it can't reach the bazaar and
/// freeze the board before the human dies.
#[test]
fn human_topping_out_loses_the_match() {
    let mut vs = VsComputer::new(7, 0); // AI = Comatose (won't reach the bazaar in time)
    for _ in 0..50_000 {
        for _ in 0..12 {
            vs.player_mut().move_left();
        }
        vs.player_mut().begin_drop();
        vs.tick(DT);
        if vs.result() != 0 {
            break;
        }
    }
    assert_eq!(vs.result(), 2, "the human topping out should lose (result == 2)");
    assert!(vs.player().is_game_over());
}
