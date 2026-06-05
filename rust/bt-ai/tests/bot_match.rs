//! Integration: two networked-bot brains (strong placement + smart weapon policy)
//! play a real two-player match through the SAME cross-player relay the server uses
//! (`bt_core::deliver_weapon` + `receive_op_score`), the way `VsComputer` wires its
//! two sides. Proves the pieces work together: both bots clear lines AND buy +
//! launch + deliver weapons. No websocket layer — pure engine.

use bt_ai::best_placement_strong;
use bt_ai::weapons::{buy_plan, launch_choice};
use bt_core::game::GameEvent;
use bt_core::weapons::WeaponToken;
use bt_core::Game;

const PLACE_EVERY: i32 = 6; // ticks between placements (brisk, for a quick test)
const LAUNCH_EVERY: i32 = 40;

struct Bot {
    committed: bool,
    place_cd: i32,
    launch_cd: i32,
    shopped: bool,
}
impl Bot {
    fn new() -> Bot {
        Bot { committed: false, place_cd: PLACE_EVERY, launch_cd: LAUNCH_EVERY, shopped: false }
    }
}

fn arsenal(g: &Game) -> [i32; 10] {
    let mut a = [-1i32; 10];
    for (i, s) in a.iter_mut().enumerate() {
        *s = g.arsenal_token(i);
    }
    a
}

fn has_spy(g: &Game) -> bool {
    g.weapon_active(WeaponToken::Ames)
        || g.weapon_active(WeaponToken::Ace)
        || g.weapon_active(WeaponToken::Condor)
}

/// Is THIS game's own board stacked high (a perfect-spy view for the test)?
fn board_high(g: &Game) -> bool {
    let b = g.board();
    let (w, h) = (b.width, b.height);
    let mut min_top = h;
    for x in 0..w {
        for y in 0..h {
            if b.occupied(x, y) {
                if y < min_top {
                    min_top = y;
                }
                break;
            }
        }
    }
    (h - min_top) as f64 >= 0.6 * h as f64
}

/// Steer + hard-drop the current piece to the strong placement.
fn place(g: &mut Game) {
    let p = match g.current_piece() {
        Some(p) => p.clone(),
        None => return,
    };
    let pl = best_placement_strong(g.board(), &p);
    for _ in 0..p.orientations.max(1) {
        match g.current_piece() {
            Some(cp) if cp.orientation != pl.orientation => g.rotate(),
            _ => break,
        }
    }
    for _ in 0..g.board().width * 2 {
        match g.current_piece().map(|cp| cp.x) {
            Some(x) if x < pl.x => g.move_right(),
            Some(x) if x > pl.x => g.move_left(),
            _ => break,
        }
    }
    g.ai_begin_drop();
}

fn shop(g: &mut Game) {
    let funds = g.score().funds;
    let ars = arsenal(g);
    let carter = g.weapon_active(WeaponToken::Carter);
    for tok in buy_plan(funds, &ars, carter) {
        g.buy_weapon(tok);
    }
    g.leave_bazaar();
}

struct Tally {
    launched: i32,
    delivered: i32,
}

/// Drain `from`'s events, route launches to `to` via the shared relay (and score
/// mirror), and reset the bot's committed flag on lock. Mirrors `VsComputer::relay`.
fn relay(from: &mut Game, to: &mut Game, bot: &mut Bot, t: &mut Tally) {
    for e in from.take_events() {
        match e {
            GameEvent::WeaponLaunched(tok) => {
                t.launched += 1;
                bt_core::deliver_weapon(from, to, tok);
                t.delivered += 1;
            }
            GameEvent::Scored { score, lines, funds } => to.receive_op_score(score, lines, funds),
            GameEvent::FundsStolen(amount) => to.add_funds(amount),
            GameEvent::Locked { .. } => bot.committed = false,
            _ => {}
        }
    }
}

fn maybe_launch(g: &mut Game, bot: &mut Bot, opp_high: bool) {
    if bot.launch_cd > 0 {
        bot.launch_cd -= 1;
        return;
    }
    let ars = arsenal(g);
    if let Some(slot) = launch_choice(&ars, has_spy(g), opp_high) {
        g.launch_weapon(slot);
        bot.launch_cd = LAUNCH_EVERY;
    } else {
        bot.launch_cd = 8;
    }
}

#[test]
fn two_bots_clear_lines_and_trade_weapons() {
    let mut a = Game::new(11);
    let mut b = Game::new(22);
    let mut ba = Bot::new();
    let mut bb = Bot::new();
    let mut ta = Tally { launched: 0, delivered: 0 };
    let mut tb = Tally { launched: 0, delivered: 0 };

    for _ in 0..8000 {
        if a.is_game_over() || b.is_game_over() {
            break;
        }

        // Bazaar barrier: both sides shop once on entry, then leave (no ticking).
        if a.is_in_bazaar() || b.is_in_bazaar() {
            if a.is_in_bazaar() && !ba.shopped {
                shop(&mut a);
                ba.shopped = true;
            }
            if b.is_in_bazaar() && !bb.shopped {
                shop(&mut b);
                bb.shopped = true;
            }
            relay(&mut a, &mut b, &mut ba, &mut ta);
            relay(&mut b, &mut a, &mut bb, &mut tb);
            continue;
        }
        ba.shopped = false;
        bb.shopped = false;

        // Place one piece per fresh spawn, on the placement cadence.
        if !ba.committed && a.current_piece().is_some() && ba.place_cd <= 0 {
            place(&mut a);
            ba.committed = true;
            ba.place_cd = PLACE_EVERY;
        } else {
            ba.place_cd -= 1;
        }
        if !bb.committed && b.current_piece().is_some() && bb.place_cd <= 0 {
            place(&mut b);
            bb.committed = true;
            bb.place_cd = PLACE_EVERY;
        } else {
            bb.place_cd -= 1;
        }

        // Launch (opp_high read perfectly off the real opponent board iff we hold a
        // live spy — exercising the smart, intel-timed path). Precompute the reads
        // so they don't overlap the mutable borrows.
        let a_spy = has_spy(&a);
        let b_spy = has_spy(&b);
        let a_high = board_high(&a);
        let b_high = board_high(&b);
        maybe_launch(&mut a, &mut ba, a_spy && b_high);
        maybe_launch(&mut b, &mut bb, b_spy && a_high);

        a.tick(16);
        b.tick(16);
        relay(&mut a, &mut b, &mut ba, &mut ta);
        relay(&mut b, &mut a, &mut bb, &mut tb);
    }

    let (la, lb) = (a.score().lines, b.score().lines);
    let launched = ta.launched + tb.launched;
    let delivered = ta.delivered + tb.delivered;
    println!(
        "A: {la} lines, B: {lb} lines | launched {launched}, delivered {delivered}"
    );

    // Both bots clear lines steadily despite trading garbage.
    assert!(la >= 10, "bot A cleared too few lines: {la}");
    assert!(lb >= 10, "bot B cleared too few lines: {lb}");
    // Weapons actually get bought + launched + delivered across the match.
    assert!(launched >= 8, "too few weapons launched: {launched}");
    assert!(delivered >= 8, "too few weapons delivered: {delivered}");
}
