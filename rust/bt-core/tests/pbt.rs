//! Property-based tests for the falling-piece engine.
//!
//! These fuzz random sequences of player inputs + clock ticks against a `Game`
//! and assert engine invariants after every step. proptest shrinks any failure
//! to a minimal operation sequence.

use bt_core::Game;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Left,
    Right,
    Rotate,
    Soft,
    Drop,
    Tick,
}

fn op() -> impl Strategy<Value = Op> {
    // Weight Tick high so pieces actually fall / slide / lock (and the
    // resume-from-slide path is exercised), with moves/rotates mixed in.
    prop_oneof![
        4 => Just(Op::Tick),
        1 => Just(Op::Left),
        1 => Just(Op::Right),
        1 => Just(Op::Rotate),
        1 => Just(Op::Soft),
        1 => Just(Op::Drop),
    ]
}

fn apply(g: &mut Game, op: &Op) {
    match op {
        Op::Left => g.move_left(),
        Op::Right => g.move_right(),
        Op::Rotate => g.rotate(),
        Op::Soft => g.soft_drop(),
        Op::Drop => g.begin_drop(),
        Op::Tick => g.tick(16),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// INVARIANT: the game's position (`piece_pos()`, used for collision and
    /// locking) must always equal the falling piece's own position (`p.x/p.y`,
    /// used for rendering and `land()`). If they diverge, a piece locks where
    /// the game checked collision but renders/lands a row away — i.e. it comes
    /// to rest in mid-air. Reproduces the replay-75037e bug.
    #[test]
    fn position_stays_synced(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..400),
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() {
                break;
            }
            apply(&mut g, o);
            if let Some(p) = g.current_piece() {
                let (gx, gy) = g.piece_pos();
                prop_assert!(
                    (gx, gy) == (p.x, p.y),
                    "position desync after {:?}: game=({}, {}) piece=({}, {})",
                    o, gx, gy, p.x, p.y
                );
            }
        }
    }
}
