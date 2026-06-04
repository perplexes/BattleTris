//! Property-based tests for the falling-piece engine.
//!
//! These fuzz random sequences of player inputs + clock ticks against a `Game`
//! and assert engine invariants after every step. proptest shrinks any failure
//! to a minimal operation sequence.

use bt_core::{Game, WeaponToken};
use proptest::prelude::*;

/// Force a lone Game into the bazaar deterministically by crossing a 20-line
/// bazaar boundary via the opponent-score mirror (combined lines 19 -> 20).
fn enter_bazaar(g: &mut Game) {
    g.receive_op_score(0, 19, 0);
    g.receive_op_score(0, 20, 0);
}

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
        let o0 = p.orientation;
        // Skip NON-rotatable pieces by their OWN metadata (`rot == 0`: Box / Die /
        // Happy / FourByFour have a 0-sized rotation sub-square), NOT by observing
        // whether `rotate()` did nothing — observing the function under test would
        // let a `rotate -> no-op` mutant satisfy the property vacuously for every
        // rotatable piece.
        if p.rot == 0 || orientations <= 1 { return Ok(()); }

        let mut expected = o0;
        // Spin a full cycle plus a bit; on an empty board at spawn every step
        // must land on the next orientation, and after `orientations` steps it
        // must return to the start (the wrap). Because this piece IS rotatable
        // (rot != 0) and has room at spawn, EVERY rotate must advance — a no-op
        // here is now a real failure, not a skip.
        for step in 1..=(orientations + 2) {
            g.rotate();
            let cur = g.current_piece().map(|p| p.orientation);
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

    /// SOFT DROP semantic: on a FRESH falling piece (empty board, spawn row),
    /// `soft_drop()` advances the piece down by EXACTLY one row (`y += delta_y`),
    /// and never moves it sideways or up. Repeated soft drops step down one row at
    /// a time until the piece reaches the floor, where the next soft drop enters
    /// the lock SLIDE instead of advancing (the piece can't go lower). A
    /// `soft_drop -> return;` no-op (or one that moves the wrong way) is caught.
    #[test]
    fn soft_drop_advances_one_row_then_slides_at_floor(seed in any::<u64>()) {
        let mut g = Game::new(seed);
        let (x0, y0) = g.piece_pos();
        prop_assert_eq!(y0, 0, "fresh piece must start at the spawn row");

        // One soft drop -> down exactly one row, same column.
        g.soft_drop();
        let (x1, y1) = g.piece_pos();
        prop_assert_eq!((x1, y1), (x0, y0 + 1),
            "soft_drop must advance y by exactly 1 (no sideways/upward move)");
        prop_assert_eq!(g.current_piece().map(|p| (p.x, p.y)), Some((x1, y1)),
            "the rendered piece must follow the soft-drop step");

        // Keep soft-dropping: each step advances y by 1 until the floor, after
        // which y stops advancing (the piece has entered the lock slide).
        let mut prev_y = y1;
        let mut reached_floor = false;
        for _ in 0..40 {
            if g.is_game_over() { break; }
            g.soft_drop();
            let yn = g.piece_pos().1;
            if yn == prev_y {
                // Can't descend further — at the floor / on a slide.
                reached_floor = true;
                break;
            }
            prop_assert_eq!(yn, prev_y + 1, "each soft_drop must step down exactly one row");
            prev_y = yn;
        }
        prop_assert!(reached_floor || g.is_game_over(),
            "the piece must eventually reach the floor (soft drop stops advancing y)");
    }

    /// HUMAN HARD-DROP scoring: the FIRST `begin_drop()` awards exactly
    /// `BT_BOARD_HGT - y` points (the further the piece still had to fall, the more
    /// it's worth — BTGame.C:729), and a SECOND `begin_drop()` (fast drop already
    /// engaged) awards NOTHING. CRUCIALLY we vary `y` by soft-dropping the piece
    /// down first, so the bonus genuinely DEPENDS on `y`: a mutant that awards a
    /// constant `BT_BOARD_HGT` (ignoring `y`) is now caught — the old fixed-y==0
    /// version couldn't tell `BT_BOARD_HGT - 0` from `BT_BOARD_HGT`. soft_drop
    /// advances `y` without engaging the fast drop, so the award is still the
    /// first-engage bonus at the descended `y`.
    #[test]
    fn begin_drop_awards_board_height_minus_y_once(
        seed in any::<u64>(),
        // How many rows to soft-drop the piece before hard-dropping (y > 0).
        descend in 0u32..12,
    ) {
        use bt_core::constants::BT_BOARD_HGT;
        let mut g = Game::new(seed);

        // Soft-drop the piece down `descend` rows (or until it can't go lower),
        // WITHOUT engaging the fast drop (so begin_drop's first-engage bonus fires).
        let mut steps = 0u32;
        for _ in 0..descend {
            let y_pre = g.piece_pos().1;
            g.soft_drop();
            if g.piece_pos().1 == y_pre { break; } // hit the floor / slide
            steps += 1;
        }
        let y = g.piece_pos().1;
        prop_assume!(g.current_piece().is_some() && !g.is_game_over());
        // y must equal the number of successful soft-drop steps (started at 0).
        prop_assert_eq!(y, steps as i32, "y must track the soft-drop descent");
        let s0 = g.score().score;

        g.begin_drop();
        let s1 = g.score().score;
        prop_assert_eq!(s1 - s0, (BT_BOARD_HGT - y) as i64,
            "first begin_drop must award BT_BOARD_HGT - y = {} - {} = {}",
            BT_BOARD_HGT, y, BT_BOARD_HGT - y);

        // A second begin_drop (fast drop already engaged) must NOT award again.
        g.begin_drop();
        prop_assert_eq!(g.score().score, s1,
            "a second begin_drop must not double-award the hard-drop bonus");
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

// ---------------------------------------------------------------------------
// BAZAAR ECONOMY oracle. Buying a weapon must DEBIT its (Carter-doubled) price
// and add it to the arsenal; selling must REFUND the price and remove it. The
// robustness/bout PBTs only gate buy/sell or assert funds stay non-negative —
// they never pin the funds delta or the arsenal change, so removing
// `self.score.funds -= price` (free weapons) survived. These pin both, including
// Carter's price doubling.
// ---------------------------------------------------------------------------
proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn bazaar_buy_then_sell_conserves_funds_and_arsenal(
        seed in any::<u64>(),
        tok_idx in 0usize..34,
        // Extra headroom so the buy is always affordable.
        extra in 0i64..5000,
    ) {
        let token = WeaponToken::ALL[tok_idx];
        let mut g = Game::new(seed);
        enter_bazaar(&mut g);
        prop_assume!(g.is_in_bazaar());

        let price = g.bazaar_price(token) as i64;
        // Seed exactly price + extra funds so the buy is affordable.
        g.add_funds(price + extra);
        let funds0 = g.score().funds;
        let qty0 = (0..10).map(|s| if g.arsenal_token(s) == tok_idx as i32 { g.arsenal_quantity(s) } else { 0 }).sum::<u16>();

        // BUY: funds drop by EXACTLY the effective price; the arsenal gains one.
        let bought = g.buy_weapon(token);
        prop_assert!(bought, "an affordable buy in the bazaar must succeed (price {})", price);
        prop_assert_eq!(g.score().funds, funds0 - price,
            "buy must debit exactly the price {} (funds {} -> {})", price, funds0, g.score().funds);
        let qty1 = (0..10).map(|s| if g.arsenal_token(s) == tok_idx as i32 { g.arsenal_quantity(s) } else { 0 }).sum::<u16>();
        prop_assert_eq!(qty1, qty0 + 1, "buy must add one of the token to the arsenal");

        // SELL: funds return to where they were; the arsenal loses one.
        let sold = g.sell_weapon(token);
        prop_assert!(sold, "selling the just-bought weapon must succeed");
        prop_assert_eq!(g.score().funds, funds0,
            "sell must refund exactly the price (funds back to {})", funds0);
        let qty2 = (0..10).map(|s| if g.arsenal_token(s) == tok_idx as i32 { g.arsenal_quantity(s) } else { 0 }).sum::<u16>();
        prop_assert_eq!(qty2, qty0, "sell must remove the bought token from the arsenal");
    }

    /// CARTER doubles the bazaar price. With Carter active, `bazaar_price` is 2x
    /// the base, and a buy debits the doubled amount. Activating Carter requires a
    /// lock to flush it, so we do that BEFORE entering the bazaar.
    #[test]
    fn carter_doubles_the_bazaar_price(
        seed in any::<u64>(),
        tok_idx in 0usize..34,
    ) {
        let token = WeaponToken::ALL[tok_idx];

        // Base price (no Carter).
        let base = {
            let mut g = Game::new(seed);
            enter_bazaar(&mut g);
            g.bazaar_price(token) as i64
        };

        // Carter active: receive it and lock to flush, THEN enter the bazaar.
        let mut g = Game::new(seed);
        g.receive_weapon(WeaponToken::Carter);
        // Flush the pending Carter by driving a lock.
        g.begin_drop();
        for _ in 0..1200 {
            g.tick(16);
            if g.weapon_active(WeaponToken::Carter) || g.is_game_over() { break; }
        }
        prop_assume!(g.weapon_active(WeaponToken::Carter) && !g.is_game_over());
        enter_bazaar(&mut g);
        prop_assume!(g.is_in_bazaar());

        prop_assert_eq!(g.bazaar_price(token) as i64, base * 2,
            "Carter must double the bazaar price ({} -> expected {})", base, base * 2);
        // And a buy debits the doubled price.
        g.add_funds(base * 2);
        let funds0 = g.score().funds;
        prop_assume!(g.buy_weapon(token));
        prop_assert_eq!(g.score().funds, funds0 - base * 2,
            "a Carter-priced buy must debit the doubled price");
    }
}
