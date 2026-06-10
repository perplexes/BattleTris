//! Ernie's strategy and economy engine, the "commando orders" system from
//! `BTComputer.C`.
//!
//! The board-placement stacker lives in [`crate`] (`eval_board` / `best_placement`).
//! This module is the other half of the original computer: deciding WHICH weapons
//! to buy in the bazaar and WHEN to launch them. The original models this as a
//! queue of `BTCOrders`, each firing when a trigger is met (the opponent's line
//! count, the bazaar count, or Ernie's own board height). Purchases form combos
//! that launch together once enough has been bought.
//!
//! What is ported faithfully: the purchase whitelist and its contextual unlocks
//! (`BTComputer.C:177-188,442-463`), the Swap board-height gate (`:552-555`), the
//! combo accounting and `BTC_COMBO -> BTC_LAUNCH` promotion (`:564-672`), the
//! order-trigger evaluation with the Mirror self-curse hold (`:804-871`), and the
//! never-launch set Hatter/FlipOut/Speedy (`BTWeaponManager.C:194-198`).
//!
//! What is not ported: the Lawyers-expiry bookkeeping order (an internal flag with
//! no launch or purchase effect), and exact move-for-move parity, which is
//! impossible because it depends on the piece stream (the RNG is not shared with
//! the 1994 build).

use bt_core::weapons::{weapon_table, WeaponToken, BT_MAX_WEAPONS};
use bt_core::{Board, Game, Rng};

/// `BT_SWAPLINE`: Ernie may buy Swap only when its own stack has risen to row 5
/// or above (a bad board worth trading away). `BTComputer.C:42,552-555`.
const BT_SWAPLINE: i32 = 5;
/// `BT_MIN_COMBO_COST`: once a combo's accumulated price reaches this, it is ready
/// to launch. `BTComputer.C:69`.
const BT_MIN_COMBO_COST: i64 = 750;
/// `BT_MAX_COMBO_COST`: a combo's accumulated price is capped here. `BTComputer.C:70`.
const BT_MAX_COMBO_COST: i64 = 1250;
/// Number of weapon tokens (the `can_purchase`/`can_launch` table size).
const N: usize = BT_MAX_WEAPONS;

/// What an order does once its trigger fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OrderKind {
    /// Launch `weapon` (held back while Ernie is Mirror-cursed).
    Launch,
    /// A bought-but-waiting combo member; promoted to [`OrderKind::Launch`] once
    /// the combo is complete and the weapon is launchable.
    Combo,
    /// Unlock buying `weapon` (a contextual `can_purchase` re-enable).
    CanPurchase,
}

/// One `BTCOrders` entry: an action plus up to three triggers. A trigger of `-1`
/// is inactive; the order fires when ANY active trigger is satisfied.
#[derive(Clone, Copy, Debug)]
struct Order {
    kind: OrderKind,
    /// Weapon index (0..34) this order acts on.
    weapon: i32,
    /// Fires when the opponent's line count reaches this. `BTCOrders::line_no_`.
    line_no: i32,
    /// Fires when the bazaar count reaches this. `BTCOrders::bazaar_no_`.
    bazaar_no: i32,
    /// Fires when Ernie's own board top reaches this height. `BTCOrders::my_line_no_`.
    my_line_no: i32,
}

/// Ernie's economy/launch engine (`BTComputer`'s commando subsystem).
#[derive(Clone, Debug)]
pub struct Strategy {
    commando: Vec<Order>,
    can_purchase: [bool; N],
    can_launch: [bool; N],
    /// The weapon being bought next, or `None` (`BT_NO_WPN`). Carries combo chains
    /// across buys (NiceDay -> Reagan, Speedy -> Speedy).
    next_weapon: Option<usize>,
    /// Accumulated price of the current combo. `-1` before the first purchase, `0`
    /// once a combo is complete and ready to launch.
    combo_cost: i64,
    /// How many bazaars have happened (the `bazaar_no_` trigger source).
    bazaar_no: i32,
    /// The opponent-line offset stamped onto new launch orders (`next_launch_`).
    next_launch: i32,
    /// Lifetime count of weapons bought, for the buy loop's progress check.
    weapons_bought: u64,
    /// Strategy RNG (the original's `rand()` weapon picks). Seeded from the game
    /// seed, separate from the engine's piece RNG so picks do not perturb pieces.
    rng: Rng,
}

/// The highest occupied row of `board` (smallest y), or `board.height` if empty.
/// Mirrors `cboard_.top_` (`BTCBoard::rescan`).
fn board_top(board: &Board) -> i32 {
    let h = board.height;
    let mut top = h;
    for x in 0..board.width {
        for y in 0..h {
            if board.occupied(x, y) {
                if y < top {
                    top = y;
                }
                break;
            }
        }
    }
    top
}

impl Strategy {
    /// A fresh strategy (`BTComputer::reset`): every weapon purchasable except the
    /// spies, Meadow, Susan, and Reagan, with one seeded order that unlocks Susan
    /// once the opponent reaches 50 lines (`BTComputer.C:177-199`).
    pub fn new(seed: u64) -> Strategy {
        use WeaponToken::*;
        let mut can_purchase = [true; N];
        for t in [Ace, Condor, Ames, Meadow, Susan, Reagan] {
            can_purchase[t.index()] = false;
        }
        let commando = vec![Order {
            kind: OrderKind::CanPurchase,
            weapon: Susan.index() as i32,
            line_no: 50,
            bazaar_no: -1,
            my_line_no: -1,
        }];
        Strategy {
            commando,
            can_purchase,
            can_launch: [true; N],
            next_weapon: None,
            combo_cost: -1,
            bazaar_no: 0,
            next_launch: 0,
            weapons_bought: 0,
            rng: Rng::new(seed),
        }
    }

    /// Carter-adjusted price of weapon index `w` (`priceWeapon`, `BTComputer.C:164`).
    fn price_weapon(w: usize, carter: bool) -> i64 {
        let p = weapon_table()[w].price as i64;
        if carter {
            p * 2
        } else {
            p
        }
    }

    /// Whether `weapon` may be picked next: it must be purchasable and not already
    /// queued as a pending Carter/Susan/Swap order (`purchaseApproved`,
    /// `BTComputer.C:528-547`).
    fn purchase_approved(&self, weapon: usize) -> bool {
        use WeaponToken::*;
        if !self.can_purchase[weapon] {
            return false;
        }
        for o in &self.commando {
            if o.weapon as usize == weapon {
                if let Some(Carter | Susan | Swap) = WeaponToken::from_index(o.weapon) {
                    return false;
                }
            }
        }
        true
    }

    /// Shop the bazaar (`goShopping`, `BTComputer.C:units around 528-672`): set the
    /// Swap height gate, then greedily buy combos within budget and queue their
    /// launch orders. Call once on bazaar entry.
    pub fn go_shopping(&mut self, game: &mut Game) {
        use WeaponToken::*;
        self.bazaar_no += 1;

        // Swap board-height gate: buy Swap only when the stack has risen to row 5
        // or above (a board worth trading away).
        self.can_purchase[Swap.index()] = board_top(game.board()) <= BT_SWAPLINE;

        let carter = game.weapon_active(Carter);

        // Outer do-while: keep buying while the last pass bought something and no
        // combo has just completed.
        for _ in 0..256 {
            let old_bought = self.weapons_bought;

            if self.next_weapon.is_none() {
                // Pick a random purchasable weapon to start a combo.
                for _ in 0..256 {
                    let n = self.rng.rand_below(N as i32) as usize;
                    if self.purchase_approved(n) {
                        self.next_weapon = Some(n);
                        break;
                    }
                }
                if self.next_weapon.is_none() {
                    break;
                }
            }

            while let Some(w) = self.next_weapon {
                let price = Self::price_weapon(w, carter);
                if price > game.score().funds {
                    break;
                }
                let tok = match WeaponToken::from_index(w as i32) {
                    Some(t) => t,
                    None => break,
                };
                if !game.buy_weapon(tok) {
                    break; // arsenal full or unaffordable
                }
                self.weapons_bought += 1;
                self.combo_cost = if self.combo_cost < 0 { price } else { self.combo_cost + price };

                // Queue the launch/combo order. SoLong/Mondale/Carter launch on
                // their own; everything else waits as a combo member.
                let kind = match tok {
                    SoLong | Mondale | Carter => OrderKind::Launch,
                    _ => OrderKind::Combo,
                };
                if tok == Reagan {
                    self.can_launch[NiceDay.index()] = true;
                }
                self.commando.push(Order {
                    kind,
                    weapon: w as i32,
                    line_no: self.next_launch,
                    bazaar_no: -1,
                    my_line_no: -1,
                });

                // Combo chains: NiceDay pulls in a Reagan; Speedy stacks itself.
                self.next_weapon = None;
                match tok {
                    NiceDay => {
                        self.can_launch[NiceDay.index()] = false;
                        self.can_purchase[NiceDay.index()] = false;
                        self.can_purchase[Reagan.index()] = true;
                        self.next_weapon = Some(Reagan.index());
                        self.next_launch += 1;
                        if self.combo_cost >= BT_MAX_COMBO_COST {
                            self.combo_cost = BT_MAX_COMBO_COST - 1;
                        }
                    }
                    Speedy => {
                        if self.combo_cost < BT_MAX_COMBO_COST {
                            self.next_weapon = Some(Speedy.index());
                        }
                    }
                    Reagan => {
                        self.can_purchase[Reagan.index()] = false;
                    }
                    _ => {}
                }

                // A combo is ready once it has cost enough (and no chain is
                // pending), or as soon as a Lazy Susan is in it.
                if (self.combo_cost >= BT_MIN_COMBO_COST && self.next_weapon.is_none())
                    || w == Susan.index()
                {
                    self.combo_cost = 0;
                }
            }

            if self.weapons_bought == old_bought || self.combo_cost == 0 {
                break;
            }
        }

        // Promote every complete combo to a launch order (gated on launchability).
        if self.combo_cost == 0 {
            for o in self.commando.iter_mut() {
                if o.kind == OrderKind::Combo && self.can_launch[o.weapon as usize] {
                    o.kind = OrderKind::Launch;
                }
            }
        }
    }

    /// Fire any orders whose trigger is now met, holding all launches while Ernie
    /// is Mirror-cursed (`activateCommando`, `BTComputer.C:804-871`). Call once per
    /// piece placement, before placing.
    pub fn activate_commando(&mut self, game: &mut Game) {
        let op_lines = game.score().op_lines;
        let top = board_top(game.board()) as i64;
        let mirror = game.weapon_active(WeaponToken::Mirror);
        // The original reschedules a later launch order by however late the
        // previous one fired, so an intended "launch B one line after A" still
        // spaces by one line when A was delayed.
        let mut prev_line: i64 = -1;

        let mut i = 0;
        while i < self.commando.len() {
            if prev_line > -1 && self.commando[i].line_no > -1 && self.commando[i].line_no as i64 > prev_line {
                self.commando[i].line_no = (self.commando[i].line_no as i64 - prev_line + op_lines) as i32;
            }
            let o = self.commando[i];
            let fires = (o.line_no > -1 && o.line_no as i64 <= op_lines)
                || (o.bazaar_no > -1 && o.bazaar_no <= self.bazaar_no)
                || (o.my_line_no > -1 && o.my_line_no as i64 >= top);
            if fires {
                match o.kind {
                    OrderKind::Launch => {
                        if !mirror {
                            if prev_line == -1 && o.line_no > -1 {
                                prev_line = o.line_no as i64;
                            }
                            self.commando.remove(i);
                            self.launch_weapon(game, o.weapon as usize);
                            continue; // index now points at the next order
                        }
                        // Mirror-cursed: hold the launch (it backfires/fizzles),
                        // leave the order for a later piece.
                    }
                    OrderKind::CanPurchase => {
                        self.can_purchase[o.weapon as usize] = true;
                        self.commando.remove(i);
                        continue;
                    }
                    OrderKind::Combo => {} // not yet promoted; leave it
                }
            }
            i += 1;
        }
    }

    /// Launch `weapon` from Ernie's arsenal. The computer never launches
    /// Hatter/FlipOut/Speedy (`BTWeaponManager.C:194-198`). Launching Reagan also
    /// imposes the economy-weapon cooldown: Reagan/Keating/NiceDay become
    /// unbuyable for 50 of the opponent's lines (`BTComputer.C:442-463`).
    fn launch_weapon(&mut self, game: &mut Game, weapon: usize) {
        use WeaponToken::*;
        let tok = match WeaponToken::from_index(weapon as i32) {
            Some(t) => t,
            None => return,
        };
        if matches!(tok, Hatter | FlipOut | Speedy) {
            return; // the computer is barred from launching these
        }
        let mut launched = false;
        for slot in 0..10usize {
            if game.arsenal_token(slot) == weapon as i32 && game.arsenal_quantity(slot) > 0 {
                game.launch_weapon(slot);
                launched = true;
                break;
            }
        }
        if launched && tok == Reagan {
            let resume = (game.score().op_lines + 50) as i32;
            self.can_purchase[Reagan.index()] = false;
            self.can_purchase[Keating.index()] = false;
            self.can_purchase[NiceDay.index()] = false;
            self.can_launch[NiceDay.index()] = false;
            for w in [Reagan, Keating, NiceDay] {
                self.commando.push(Order {
                    kind: OrderKind::CanPurchase,
                    weapon: w.index() as i32,
                    line_no: resume,
                    bazaar_no: -1,
                    my_line_no: -1,
                });
            }
        }
    }

    /// Whether weapon index `w` is currently purchasable (test/inspection helper).
    #[cfg(test)]
    fn can_purchase(&self, w: usize) -> bool {
        self.can_purchase[w]
    }

    /// Number of pending orders (test/inspection helper).
    #[cfg(test)]
    fn pending_orders(&self) -> usize {
        self.commando.len()
    }

    /// Queue an immediate launch order for `weapon` (test helper, bypassing the
    /// bazaar so launch behavior can be tested in isolation).
    #[cfg(test)]
    fn queue_launch(&mut self, weapon: WeaponToken) {
        self.commando.push(Order {
            kind: OrderKind::Launch,
            weapon: weapon.index() as i32,
            line_no: 0,
            bazaar_no: -1,
            my_line_no: -1,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bt_core::game::GameEvent;

    /// Drop the current piece and tick until it locks, to arm a queued weapon.
    fn lock(g: &mut Game) {
        g.begin_drop();
        for _ in 0..400 {
            g.tick(16);
            if g.is_game_over() || g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
                return;
            }
        }
    }

    fn launched(g: &mut Game) -> Vec<WeaponToken> {
        g.take_events()
            .into_iter()
            .filter_map(|e| match e {
                GameEvent::WeaponLaunched(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn whitelist_blocks_spies_meadow_susan_reagan_initially() {
        use WeaponToken::*;
        let s = Strategy::new(1);
        for t in [Ace, Condor, Ames, Meadow, Susan, Reagan] {
            assert!(!s.can_purchase(t.index()), "{t:?} must NOT be purchasable at start");
        }
        for t in [RiseUp, Mondale, Keating, Bottle, Hatter, Carter] {
            assert!(s.can_purchase(t.index()), "{t:?} must be purchasable at start");
        }
    }

    #[test]
    fn susan_purchase_unlocks_at_opponent_line_50() {
        let mut s = Strategy::new(1);
        let mut g = Game::new(1);
        assert!(!s.can_purchase(WeaponToken::Susan.index()));
        // Below the threshold: the seeded unlock order does not fire.
        g.receive_op_score(0, 49, 0);
        s.activate_commando(&mut g);
        assert!(!s.can_purchase(WeaponToken::Susan.index()), "still locked below 50 opponent lines");
        // At the threshold: the order fires and unlocks the purchase.
        g.receive_op_score(0, 50, 0);
        s.activate_commando(&mut g);
        assert!(s.can_purchase(WeaponToken::Susan.index()), "unlocked at 50 opponent lines");
    }

    #[test]
    fn launches_a_normal_ordered_weapon() {
        let mut s = Strategy::new(1);
        let mut g = Game::new(1);
        g.grant_weapon(WeaponToken::RiseUp);
        s.queue_launch(WeaponToken::RiseUp);
        let before = s.pending_orders();
        s.activate_commando(&mut g);
        assert_eq!(launched(&mut g), vec![WeaponToken::RiseUp], "the ordered RiseUp launches");
        assert_eq!(s.pending_orders(), before - 1, "the fired order is removed");
    }

    #[test]
    fn never_launches_hatter_flipout_speedy() {
        use WeaponToken::*;
        for t in [Hatter, FlipOut, Speedy] {
            let mut s = Strategy::new(1);
            let mut g = Game::new(1);
            g.grant_weapon(t);
            s.queue_launch(t);
            let before = s.pending_orders();
            s.activate_commando(&mut g);
            assert!(launched(&mut g).is_empty(), "{t:?} must not be launched by the computer");
            assert_eq!(s.pending_orders(), before - 1, "the order is still consumed (fired but ignored)");
        }
    }

    #[test]
    fn holds_its_own_launches_while_mirror_cursed() {
        let mut s = Strategy::new(1);
        let mut g = Game::new(1);
        // Arm Mirror on Ernie's own board.
        g.receive_weapon(WeaponToken::Mirror);
        lock(&mut g);
        assert!(g.weapon_active(WeaponToken::Mirror), "Mirror is active on Ernie");

        g.grant_weapon(WeaponToken::RiseUp);
        s.queue_launch(WeaponToken::RiseUp);
        let before = s.pending_orders();
        s.activate_commando(&mut g);
        assert!(launched(&mut g).is_empty(), "a Mirror-cursed Ernie holds its own launch");
        assert_eq!(s.pending_orders(), before, "the held order stays pending for later");
    }
}
