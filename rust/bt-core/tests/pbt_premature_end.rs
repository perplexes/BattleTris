//! Property-based hunt for PREMATURE GAME ENDINGS: weapon combinations that top a
//! player out while the board should still have room.
//!
//! Motivation: an online match ended with one side losing right after a flurry of
//! offensive weapons, with no visible top-out on that board. A top-out is only
//! legitimate when a freshly spawned piece collides at the spawn origin. Two ways
//! that can go wrong under a weapon combo, and one invariant each:
//!
//!   (P1) SPAWN-ORIGIN CONSISTENCY. Upbyside is the only state that moves the spawn
//!        origin (spawn at the bottom, pieces rise: `def_y = BT_BOARD_HGT-4`,
//!        `delta_y = -1`); every other state leaves the defaults. The active flag
//!        and the spawn origin are set together in `apply_weapon_on` /
//!        `apply_weapon_off`, so they must never disagree. If they ever do, a piece
//!        spawns at the wrong end of the board and tops out into a stack the player
//!        never sees, which is exactly the "lost with no losing condition" report.
//!
//!   (P2) FAIR TOP-OUT. When a side does top out, its spawn footprint (the 4x4 box
//!        at the spawn origin, rows `[def_y, def_y+4)` around `def_x`) must actually
//!        hold a blocking cell. A spawn fails iff one of those cells is filled, so a
//!        top-out fired while that footprint is empty means the game ended without a
//!        blocker under the spawn, i.e. a spurious end.
//!
//! The driver runs a real `Versus` (the same match engine the server uses), with
//! BOTH sides playing pieces AND firing weapons at each other, every weapon routed
//! through its true relay path: launch -> `WeaponLaunched` event -> relay. That
//! covers paths a one-directional `deliver_weapon` harness cannot reach: spies
//! (filtered to the launcher), Susan (arsenal swap), Keating (funds), and crucially
//! Mirror, whose curse backfires the cursed launcher's own weapons onto itself, the
//! prime "I fired a weapon and *I* lost" case. All 34 weapons are in the pool, and
//! both boards are checked every step.

use bt_core::constants::{BT_BOARD_HGT, BT_BOARD_WTH, BT_DEFAULT_X, BT_DEFAULT_Y, BT_PIECE_HEIGHT};
use bt_core::versus::Side;
use bt_core::{Game, Versus, WeaponToken};
use proptest::prelude::*;

/// Total weapon tokens (0..N). The whole pool is in play: board-shapers, orientation
/// flips, piece weapons, funds, spies, Susan, and Mirror. The ones with no board
/// effect still exercise their relay route and must leave the spawn invariants
/// intact.
const N_WEAPONS: i32 = 34;

#[derive(Debug, Clone)]
enum Op {
    /// Advance the whole match one frame (16 ms): both boards tick, then the relay
    /// resolves any cross-player effects.
    Tick,
    /// A movement input on one side (`true` = side A). Both sides play, so both
    /// stacks grow and either can be the one that tops out.
    Left(bool),
    Right(bool),
    Rotate(bool),
    Drop(bool),
    /// `side` fires weapon token `tok` (0..N_WEAPONS) at the opponent, through the
    /// real launch + relay path. `side` is `true` for A.
    Launch { side: bool, tok: i32 },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        14 => Just(Op::Tick),
        2 => any::<bool>().prop_map(Op::Left),
        2 => any::<bool>().prop_map(Op::Right),
        2 => any::<bool>().prop_map(Op::Rotate),
        3 => any::<bool>().prop_map(Op::Drop),
        8 => (any::<bool>(), 0..N_WEAPONS).prop_map(|(side, tok)| Op::Launch { side, tok }),
    ]
}

fn side_of(a: bool) -> Side {
    if a {
        Side::A
    } else {
        Side::B
    }
}

/// Render a board as ASCII (`#` filled, `.` empty) for failure messages.
fn board_ascii(g: &Game) -> String {
    let b = g.board();
    let mut s = String::new();
    for y in 0..b.height {
        s.push_str(&format!("{y:2} "));
        for x in 0..b.width {
            s.push(if b.get(x, y).is_some() { '#' } else { '.' });
        }
        s.push('\n');
    }
    s
}

/// The active weapons on a game, for failure messages.
fn active_list(g: &Game) -> String {
    (0..N_WEAPONS)
        .filter_map(WeaponToken::from_index)
        .filter(|t| g.weapon_active(*t))
        .map(|t| format!("{t:?}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Fire `tok` from `side` at the opponent, through the real path: grant it into the
/// arsenal, launch the slot it lands in, then run the relay (a zero-length tick
/// resolves the launch event without advancing either clock). A grant that does not
/// fit (arsenal full of distinct kinds) is skipped.
fn launch(v: &mut Versus, side: Side, tok: WeaponToken) {
    let g = v.game_mut(side);
    g.grant_weapon(tok);
    let slot = (0..10).find(|&s| g.arsenal_token(s) == tok.index() as i32);
    if let Some(slot) = slot {
        g.launch_weapon(slot);
        // Resolve the launch through the relay without ticking either board.
        v.tick(0);
    }
}

fn apply(v: &mut Versus, op: &Op) {
    match op {
        Op::Tick => v.tick(16),
        Op::Left(a) => v.game_mut(side_of(*a)).move_left(),
        Op::Right(a) => v.game_mut(side_of(*a)).move_right(),
        Op::Rotate(a) => v.game_mut(side_of(*a)).rotate(),
        Op::Drop(a) => v.game_mut(side_of(*a)).begin_drop(),
        Op::Launch { side, tok } => {
            if let Some(token) = WeaponToken::from_index(*tok) {
                launch(v, side_of(*side), token);
            }
        }
    }
}

/// (P1) The spawn origin must agree with the Upbyside flag. Returns an error string
/// on a mismatch.
fn check_spawn_consistency(g: &Game, who: &str, ops: &[Op]) -> Result<(), String> {
    let (dx, dy, ddy) = g.spawn_origin();
    let up = g.weapon_active(WeaponToken::Upbyside);
    let ok = if up {
        dx == BT_DEFAULT_X && dy == BT_BOARD_HGT - 4 && ddy == -1
    } else {
        dx == BT_DEFAULT_X && dy == BT_DEFAULT_Y && ddy == 1
    };
    if ok {
        return Ok(());
    }
    Err(format!(
        "SPAWN ORIGIN DESYNC on {who}: Upbyside={up} but spawn_origin=(def_x={dx}, def_y={dy}, \
         delta_y={ddy}); expected {}.\nactive=[{}]\nops={:?}\nboard:\n{}",
        if up { "(5, 24, -1)" } else { "(5, 0, 1)" },
        active_list(g),
        ops,
        board_ascii(g),
    ))
}

/// (P2) A topped-out game must have a blocking cell in the spawn footprint. Only
/// meaningful once `is_game_over()`.
fn check_fair_topout(g: &Game, who: &str, ops: &[Op]) -> Result<(), String> {
    if !g.is_game_over() {
        return Ok(());
    }
    let b = g.board();
    // A piece spawns with its 8x8 box origin at (def_x - rot/2, def_y). The filled
    // cells live somewhere in that box, so they occupy board rows [def_y, def_y+8)
    // and some columns, in BOTH orientations (Upbyside only moves def_y to
    // BT_BOARD_HGT-4 and flips the fall direction). rot is unknown post-mortem and
    // the box is 8 wide, so the blocker can sit anywhere across the width; the sound,
    // false-positive-free check is therefore the whole spawn-row band across every
    // column. A spawn fails iff one of the piece's cells is filled, so a legitimate
    // top-out always blocks somewhere in these rows. An entirely empty band at game
    // over means nothing could block the spawn, i.e. a spurious end.
    let (_, def_y, _) = g.spawn_origin();
    let y_lo = def_y.max(0);
    let y_hi = (def_y + BT_PIECE_HEIGHT as i32).min(BT_BOARD_HGT);
    let blocked = (y_lo..y_hi).any(|y| (0..BT_BOARD_WTH).any(|x| b.get(x, y).is_some()));
    if blocked {
        return Ok(());
    }
    Err(format!(
        "PREMATURE TOP-OUT on {who}: game over but the spawn rows {}..{} (all columns) are empty, \
         so nothing could block the spawn.\nactive=[{}] spawn_origin={:?}\nops={:?}\nboard:\n{}",
        y_lo,
        y_hi,
        active_list(g),
        g.spawn_origin(),
        ops,
        board_ascii(g),
    ))
}

/// Check both invariants on both sides. Returns an error string for the first
/// violation found; once a side is over, P2 is the deciding check.
fn check(v: &Versus, ops: &[Op]) -> Result<(), String> {
    for (a, who) in [(true, "A"), (false, "B")] {
        let g = v.game(side_of(a));
        check_spawn_consistency(g, who, ops)?;
        check_fair_topout(g, who, ops)?;
    }
    Ok(())
}

proptest! {
    // A wide search: many medium op streams, every weapon and both directions in play.
    // Heavier than the usual suites because the failure is rare and combination-deep;
    // crank further with PROPTEST_CASES=N for a deeper sweep.
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// No weapon combination, fired by either side (including Mirror backfires onto
    /// the launcher), drives a spawn-origin desync (P1) or a top-out into an empty
    /// spawn footprint (P2) on either board.
    #[test]
    fn weapon_combinations_never_end_the_game_prematurely(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        ops in prop::collection::vec(op(), 0..400),
    ) {
        let mut v = Versus::new(seed_a, seed_b);

        let mut applied: Vec<Op> = Vec::with_capacity(ops.len());
        for o in &ops {
            apply(&mut v, o);
            applied.push(o.clone());

            if let Err(e) = check(&v, &applied) {
                prop_assert!(false, "{e}");
            }
            // Once the match is decided, the deciding top-out has been checked; stop.
            if v.is_over() {
                break;
            }
        }
    }
}
