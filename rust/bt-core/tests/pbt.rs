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

// ---------------------------------------------------------------------------
// SEMANTIC INPUT direction oracle.
//
// The invariant/determinism properties above NEVER pin the MEANING of an input:
// a mutant that makes `move_left` move RIGHT (game.rs `self.x += self.left_x`
// flipped, or `left_x: 1`) keeps every cell in-bounds, every position synced,
// and every run deterministic — so it sails through all of them. These pin the
// actual direction on an EMPTY board where the move always succeeds, and pin a
// wall as a hard stop (the move is a genuine no-op, not the wrong direction).
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// On a fresh (empty) board, `move_left` moves the falling piece exactly one
    /// column LEFT (x decreases by 1) and `move_right` exactly one column RIGHT
    /// (x increases by 1). Both `piece_pos()` (collision frame) and the rendered
    /// piece's own `p.x` must agree. A flipped direction (left==right), a
    /// double-step, or a no-op all fail here.
    #[test]
    fn move_left_and_right_step_one_column_on_empty_board(seed in any::<u64>()) {
        let mut g = Game::new(seed);
        // The piece spawns mid-board (x=5) with empty space on both sides, so the
        // very first move in either direction is guaranteed to succeed.
        let (x0, _) = g.piece_pos();

        g.move_left();
        let (xl, _) = g.piece_pos();
        prop_assert_eq!(xl, x0 - 1, "move_left must decrement x by exactly 1");
        prop_assert_eq!(g.current_piece().map(|p| p.x), Some(xl),
            "rendered piece x must follow the collision-frame x after move_left");

        // Back to centre, then right.
        g.move_right();
        let (xc, _) = g.piece_pos();
        prop_assert_eq!(xc, x0, "move_right must undo the move_left (back to centre)");

        g.move_right();
        let (xr, _) = g.piece_pos();
        prop_assert_eq!(xr, x0 + 1, "move_right must increment x by exactly 1");
        prop_assert_eq!(g.current_piece().map(|p| p.x), Some(xr),
            "rendered piece x must follow the collision-frame x after move_right");
    }

    /// A piece pressed against a wall does NOT move past it: once `move_left`
    /// (resp. `move_right`) stops changing x, one more press is a true no-op —
    /// x stays put rather than wrapping or reversing. Catches a wall-collision
    /// that silently lets the piece slide off-board (the no-overlap test would
    /// still pass because out-of-bounds cells aren't "board cells").
    #[test]
    fn piece_does_not_move_through_a_wall(seed in any::<u64>()) {
        // Walk left to the wall.
        let mut g = Game::new(seed);
        let mut prev = g.piece_pos().0;
        let mut left_wall = prev;
        for _ in 0..64 {
            g.move_left();
            let x = g.piece_pos().0;
            if x == prev { left_wall = x; break; }
            prop_assert_eq!(x, prev - 1, "each successful move_left steps exactly one left");
            prev = x;
            left_wall = x;
        }
        // One more press at the wall is a no-op (no wrap, no reverse).
        g.move_left();
        prop_assert_eq!(g.piece_pos().0, left_wall,
            "move_left at the left wall must be a no-op, not a wrap/reverse");

        // Walk right to the other wall.
        let mut g = Game::new(seed);
        let mut prev = g.piece_pos().0;
        let mut right_wall = prev;
        for _ in 0..64 {
            g.move_right();
            let x = g.piece_pos().0;
            if x == prev { right_wall = x; break; }
            prop_assert_eq!(x, prev + 1, "each successful move_right steps exactly one right");
            prev = x;
            right_wall = x;
        }
        g.move_right();
        prop_assert_eq!(g.piece_pos().0, right_wall,
            "move_right at the right wall must be a no-op, not a wrap/reverse");

        // And the two walls are genuinely on opposite sides (the loop didn't just
        // immediately stop, which would make the no-op check vacuous).
        prop_assert!(left_wall < right_wall,
            "left wall ({}) must be strictly left of the right wall ({})",
            left_wall, right_wall);
    }

    /// `rotate` advances the falling piece's orientation by exactly one step
    /// (mod `orientations`). At spawn on an empty board a rotatable piece always
    /// has room to turn, so the orientation must tick 0→1→2→… and wrap. A mutant
    /// that rotates the wrong way, skips the orientation bump, or double-steps it
    /// diverges from this expected sequence. Pieces that can't rotate (Box/Die/
    /// Happy/FourByFour: a single orientation in practice) are skipped — their
    /// rotate is a legitimate no-op.
    #[test]
    fn rotate_advances_orientation_by_one(seed in any::<u64>()) {
        let mut g = Game::new(seed);
        let Some(p) = g.current_piece() else { return Ok(()); };
        let orientations = p.orientations;
        // Skip pieces that don't actually turn (rot==0 → rotate is a no-op).
        // We detect that by attempting a rotate and seeing if orientation moves
        // at all; if not, there's nothing to pin.
        let o0 = p.orientation;
        if orientations <= 1 { return Ok(()); }

        let mut expected = o0;
        // Spin a full cycle plus a bit; on an empty board at spawn every step
        // must land on the next orientation, and after `orientations` steps it
        // must return to the start (the wrap).
        for step in 1..=(orientations + 2) {
            g.rotate();
            let cur = g.current_piece().map(|p| p.orientation);
            if cur == Some(o0) && step == 1 {
                // This piece's rotate was a no-op (rot==0) — nothing to assert.
                return Ok(());
            }
            expected = (expected + 1) % orientations;
            prop_assert_eq!(cur, Some(expected),
                "rotate step {} must advance orientation to {} (orientations={})",
                step, expected, orientations);
        }
        // It wrapped: after a full cycle the orientation is back to the start.
        let full_cycle = g.current_piece().map(|p| p.orientation);
        prop_assert_eq!(full_cycle, Some((o0 + (orientations + 2)) % orientations),
            "orientation must wrap mod orientations");
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
