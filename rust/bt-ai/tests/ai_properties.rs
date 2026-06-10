//! Property / invariant tests for Ernie.
//!
//! Where the oracle tests pin single values, these pin *relationships* that
//! must hold across a whole game — the kind of structural invariant that turns
//! a 2x scoring error or a t=0 side effect into a guaranteed failure instead of
//! a number nobody was asserting on.

use bt_ai::{Computer, VsComputer};
use bt_core::constants::BT_BOARD_HGT;
use bt_core::game::GameEvent;
use bt_core::Game;

/// Ernie's flat per-piece score (BTComputer.C:1255).
const FLAT: i64 = (BT_BOARD_HGT / 2) as i64; // 14

/// THE invariant the Ernie bug violated: after N placed pieces, Ernie's score
/// is *exactly* `14 * N` — because the only thing that touches `score.score` is
/// the flat per-piece award. A height bonus (the bug) makes this `28 * N`; a
/// constructor placement makes it `14 * (N+1)`. Either way, this fails.
#[test]
fn ai_score_is_exactly_flat_times_pieces() {
    let mut g = Game::new(98_765);
    let mut ernie = Computer::new(1);
    let mut pieces: i64 = 0;

    for _ in 0..80 {
        if g.is_game_over() || g.current_piece().is_none() {
            break;
        }
        ernie.take_turn(&mut g); // rotate / slide / ai_begin_drop (+14, exactly once)

        // Tick until this piece locks.
        let mut locked = false;
        for _ in 0..400 {
            g.tick(16);
            if g
                .take_events()
                .iter()
                .any(|e| matches!(e, GameEvent::Locked { .. }))
            {
                locked = true;
                break;
            }
            if g.is_game_over() || g.is_in_bazaar() {
                break;
            }
        }
        if !locked {
            break; // topped out mid-fall, or froze at the bazaar — stop cleanly
        }
        pieces += 1;
        assert_eq!(
            g.score().score,
            FLAT * pieces,
            "after {pieces} pieces Ernie must have banked exactly 14*pieces \
             (BTComputer.C:1255); a height bonus would read 28*pieces"
        );
    }

    assert!(pieces >= 10, "expected a healthy run of pieces, only placed {pieces}");
}

/// Ernie's score is *independent of drop height* (it's a flat constant). The
/// human bonus, by contrast, shrinks one point per row of head-start. This
/// pins the structural difference between the two scoring paths — the bug was
/// Ernie being routed onto the human (height-dependent) path.
#[test]
fn ai_score_is_independent_of_drop_height_human_is_not() {
    let ai_at = |drops: usize| {
        let mut g = Game::new(7);
        for _ in 0..drops {
            g.soft_drop();
        }
        g.ai_begin_drop();
        g.score().score
    };
    assert_eq!(ai_at(0), FLAT);
    assert_eq!(ai_at(5), FLAT, "Ernie's flat score does not move with height");

    let human_at = |drops: usize| {
        let mut g = Game::new(7);
        for _ in 0..drops {
            g.soft_drop();
        }
        g.begin_drop();
        g.score().score
    };
    assert!(
        human_at(0) > human_at(5),
        "the human bonus shrinks as the piece starts lower"
    );
    assert_eq!(
        human_at(0) - human_at(5),
        5,
        "exactly one point lost per row of forfeited head-start (BT_BOARD_HGT - y)"
    );
}

/// A constructor must not advance game state. `VsComputer::new` previously
/// steered + dropped Ernie's first piece, banking score before the first tick;
/// this guards that whole class of t=0 side effects, not just the score.
#[test]
fn vs_computer_constructor_is_pure() {
    let mut vs = VsComputer::new(42, 0); // Comatose

    assert_eq!(vs.ai().score().score, 0, "AI scores nothing before the first tick");
    assert_eq!(vs.player().score().score, 0, "player scores nothing either");

    {
        let b = vs.ai().board();
        let filled = (0..b.height)
            .flat_map(|y| (0..b.width).map(move |x| (x, y)))
            .filter(|&(x, y)| b.get(x, y).is_some())
            .count();
        assert_eq!(filled, 0, "Ernie's board must be empty before the first tick");
    }

    assert!(
        vs.drain_events().is_empty(),
        "no events may be produced before the first tick"
    );
}

/// Same seed + level ⇒ bit-identical trajectory. Determinism is the bedrock the
/// snapshot / differential tests stand on; assert it directly.
#[test]
fn vs_computer_is_deterministic() {
    let run = |seed: u64, level: usize| {
        let mut vs = VsComputer::new(seed, level);
        vs.player_mut().set_paused(true); // isolate Ernie
        for _ in 0..4_000 {
            vs.tick(16);
        }
        (vs.ai().score(), vs.player().score(), vs.result())
    };
    assert_eq!(run(123, 9), run(123, 9), "same seed+level must replay identically");
}

/// Across a run, Ernie's score and line count are monotonically non-decreasing
/// (no weapon or relay quietly subtracts from them).
#[test]
fn ai_score_and_lines_never_decrease() {
    let mut vs = VsComputer::new(2024, 10);
    vs.player_mut().set_paused(true);

    let mut last_score = 0i64;
    let mut last_lines = 0i64;
    for _ in 0..6_000 {
        vs.tick(16);
        let s = vs.ai().score();
        assert!(s.score >= last_score, "AI score went backwards: {} -> {}", last_score, s.score);
        assert!(s.lines >= last_lines, "AI lines went backwards: {} -> {}", last_lines, s.lines);
        last_score = s.score;
        last_lines = s.lines;
        if vs.result() != 0 {
            break;
        }
    }
}
