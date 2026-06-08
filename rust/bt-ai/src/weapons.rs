//! Weapon strategy for the networked bots (`bt-bot`): which weapons to buy in the
//! bazaar and which to launch given what we can see of the opponent.
//!
//! The faithful single-player Ernie (`vs.rs`) buys five arbitrary weapons and fires
//! the first non-empty slot every four seconds. The online bots use a smarter policy
//! instead: stock a spy (for intel) plus good-value offensive weapons, then activate
//! the spy and time launches. Board-raisers fire when the opponent is stacked high;
//! ongoing harassment and fund-drains fire otherwise.
//!
//! Tokens here are the protocol indices `Game::arsenal_token` exposes (positions in
//! `WeaponToken::ALL`); `token_from_index` maps them back.

use bt_core::weapons::{weapon_table, WeaponToken};

/// Map an arsenal slot's protocol index (`Game::arsenal_token`, -1 = empty) to a
/// token.
pub fn token_from_index(idx: i32) -> Option<WeaponToken> {
    if idx < 0 {
        return None;
    }
    WeaponToken::ALL.get(idx as usize).copied()
}

/// Strategic class of a weapon from the attacker's point of view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WClass {
    /// Instantly raises the victim's board. Most effective when the opponent is
    /// already stacked high, as it tips them over the top.
    Garbage,
    /// Ongoing harassment: bad pieces, scrambled controls, extra speed.
    Harass,
    /// Drains or denies the victim's funds.
    Economy,
    /// A spy: LAUNCH it to activate intel (reveals the opponent's board to us).
    Spy,
    /// Never launch. These weapons either help the victim (Meadow halves their
    /// gravity; NiceDay gives beans; Missing removes a victim block), are
    /// double-edged (Swap/Susan trade boards/arsenals), or are defensive/cosmetic
    /// (Mirror/Gimp).
    Skip,
}

/// Classify a weapon for bot strategy. Exhaustive over all 34 `WeaponToken`s.
pub fn class(t: WeaponToken) -> WClass {
    use WeaponToken::*;
    match t {
        RiseUp | Lawyers | Bottle | Blind | PieceIt | Bug | FallOut | FourByFour
        | Twilight => WClass::Garbage,
        FearedWeird | Hatter | Speedy | SoLong | NoDice | Slick | Broken | Force
        | NoSlide | Upbyside | FlipOut => WClass::Harass,
        Mondale | Keating | Reagan | Carter => WClass::Economy,
        Ames | Ace | Condor => WClass::Spy,
        Swap | Susan | Mirror | Meadow | NiceDay | Missing | Gimp => WClass::Skip,
    }
}

/// Buy-priority: good-value offensive and economy weapons, roughly best-first. The
/// spy is handled separately (bought first when we hold none). `Skip`-class weapons
/// never appear here.
const BUY_PRIORITY: &[WeaponToken] = &[
    WeaponToken::RiseUp,      // 75:  cheap instant raise
    WeaponToken::Upbyside,    // 125: flip screen + reverse controls (dur 10)
    WeaponToken::Bottle,      // 150: bottleneck walls (dur 10)
    WeaponToken::Mondale,     // 150: 30% tax over a long duration
    WeaponToken::PieceIt,     // 100: drops in a stray block
    WeaponToken::SoLong,      // 100: deprive of long pieces
    WeaponToken::Speedy,      // 275: double their speed
    WeaponToken::Broken,      // 325: same piece over and over
    WeaponToken::Force,       // 325: cleared rows don't collapse
    WeaponToken::Bug,         // 320: invisible stray block
    WeaponToken::Lawyers,     // 350: raise per line we clear (dur 5)
    WeaponToken::Hatter,      // 375: pieces never stop spinning
    WeaponToken::Blind,       // 400: bomb a region
    WeaponToken::FearedWeird, // 400: disjointed pieces
    WeaponToken::FourByFour,  // 425: 4x4 hollow box
    WeaponToken::Keating,     // 425: seize all their funds
    WeaponToken::Twilight,    // 450: their whole board goes invisible
    WeaponToken::Slick,       // 650: pieces slide endlessly
    WeaponToken::NoDice,      // 600: deprive of square pieces
];

fn price_of(t: WeaponToken, carter: bool) -> i64 {
    let p = weapon_table()[t.index()].price as i64;
    if carter {
        p * 2
    } else {
        p
    }
}

fn arsenal_has_spy(arsenal: &[i32]) -> bool {
    arsenal
        .iter()
        .filter_map(|&t| token_from_index(t))
        .any(|t| class(t) == WClass::Spy)
}

/// Decide what to buy this bazaar visit (in order), given current `funds`, the
/// 10-slot `arsenal` (token indices, -1 = empty), and whether Carter is doubling
/// prices. Buys a spy first (for intel) when we hold none, then greedily stocks the
/// priority list, diversifying first and then duplicating cheap strong weapons, until
/// the arsenal is full or nothing else is affordable. The engine independently
/// enforces affordability and capacity, so an over-eager plan cannot overspend.
pub fn buy_plan(funds: i64, arsenal: &[i32], carter: bool) -> Vec<WeaponToken> {
    let mut budget = funds;
    let mut slots = arsenal.iter().filter(|&&t| t < 0).count() as i32;
    let mut plan: Vec<WeaponToken> = Vec::new();

    // 1. A spy for launch-timing intel. Prefer Ace (good accuracy, mid price), then
    //    Ames (cheap), then Condor (priciest). Only if we don't already hold one.
    if !arsenal_has_spy(arsenal) {
        for spy in [WeaponToken::Ace, WeaponToken::Ames, WeaponToken::Condor] {
            if slots > 0 && budget >= price_of(spy, carter) {
                budget -= price_of(spy, carter);
                slots -= 1;
                plan.push(spy);
                break;
            }
        }
    }

    // 2. Greedily buy down the priority list; repeat to spend remaining funds (the
    //    second pass onward allows duplicates of the cheap, affordable picks).
    loop {
        let mut bought_any = false;
        for &t in BUY_PRIORITY {
            if slots <= 0 {
                break;
            }
            let p = price_of(t, carter);
            if budget >= p {
                budget -= p;
                slots -= 1;
                plan.push(t);
                bought_any = true;
            }
        }
        if !bought_any || slots <= 0 {
            break;
        }
    }
    plan
}

/// Choose an arsenal slot to LAUNCH (or `None` to hold this beat).
///
/// - `arsenal`: the 10 slot token indices (-1 = empty).
/// - `spy_active`: a spy of ours is currently revealing the opponent.
/// - `opp_high`: the opponent's stack is dangerously tall (only known via a spy).
///
/// Activates a spy first (when we own one and none is active), then fires instant
/// board-raisers when the opponent is high, otherwise harassment / fund-drains.
/// Never launches a `Skip`-class weapon (those help the victim or backfire).
pub fn launch_choice(arsenal: &[i32], spy_active: bool, opp_high: bool) -> Option<usize> {
    let find = |want: WClass| -> Option<usize> {
        arsenal
            .iter()
            .position(|&t| token_from_index(t).map(class) == Some(want))
    };

    // 1. No active spy but we own one → activate it for intel.
    if !spy_active {
        if let Some(s) = find(WClass::Spy) {
            return Some(s);
        }
    }
    // 2. Opponent stacked high → tip them over with instant garbage.
    if opp_high {
        if let Some(s) = find(WClass::Garbage) {
            return Some(s);
        }
    }
    // 3. Otherwise harass, then drain, then garbage as a fallback.
    for cls in [WClass::Harass, WClass::Economy, WClass::Garbage] {
        if let Some(s) = find(cls) {
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(t: WeaponToken) -> i32 {
        t.index() as i32
    }

    fn empty_arsenal() -> [i32; 10] {
        [-1; 10]
    }

    #[test]
    fn every_token_is_classified_and_only_skips_help_the_victim() {
        // Exhaustive (the match would fail to compile otherwise): assert the
        // known "helps the victim" weapons land in Skip.
        for t in [WeaponToken::Meadow, WeaponToken::NiceDay, WeaponToken::Missing] {
            assert_eq!(class(t), WClass::Skip, "{t:?} helps the victim");
        }
        assert_eq!(class(WeaponToken::Mirror), WClass::Skip);
        assert_eq!(class(WeaponToken::Swap), WClass::Skip);
        assert_eq!(class(WeaponToken::Ames), WClass::Spy);
        assert_eq!(class(WeaponToken::RiseUp), WClass::Garbage);
    }

    #[test]
    fn buy_plan_takes_a_spy_first_when_affordable() {
        let plan = buy_plan(1000, &empty_arsenal(), false);
        assert!(!plan.is_empty());
        assert!(
            plan.iter().any(|t| class(*t) == WClass::Spy),
            "should stock a spy: {plan:?}"
        );
    }

    #[test]
    fn buy_plan_never_exceeds_funds_or_slots() {
        let funds = 800i64;
        let plan = buy_plan(funds, &empty_arsenal(), false);
        let spent: i64 = plan.iter().map(|t| price_of(*t, false)).sum();
        assert!(spent <= funds, "spent {spent} > funds {funds}: {plan:?}");
        assert!(plan.len() <= 10, "bought more than 10: {plan:?}");
        // No Skip-class weapon should ever be bought.
        assert!(plan.iter().all(|t| class(*t) != WClass::Skip));
    }

    #[test]
    fn buy_plan_respects_existing_slots() {
        // 9 slots already full → at most one buy.
        let mut ars = [idx(WeaponToken::RiseUp); 10];
        ars[9] = -1;
        let plan = buy_plan(5000, &ars, false);
        assert!(plan.len() <= 1, "only one free slot: {plan:?}");
    }

    #[test]
    fn buy_plan_broke_buys_nothing() {
        let plan = buy_plan(10, &empty_arsenal(), false);
        assert!(plan.is_empty(), "can't afford anything: {plan:?}");
    }

    #[test]
    fn launch_activates_a_spy_before_attacking() {
        let mut ars = empty_arsenal();
        ars[0] = idx(WeaponToken::Speedy); // an offensive weapon
        ars[1] = idx(WeaponToken::Ames); // and a spy
        // No spy active yet → launch the spy (slot 1) for intel.
        assert_eq!(launch_choice(&ars, false, false), Some(1));
    }

    #[test]
    fn launch_prefers_garbage_when_opponent_is_high() {
        let mut ars = empty_arsenal();
        ars[0] = idx(WeaponToken::Speedy); // Harass
        ars[1] = idx(WeaponToken::RiseUp); // Garbage
        // Spy already active, opponent stacked high → fire the raiser (slot 1).
        assert_eq!(launch_choice(&ars, true, true), Some(1));
        // Opponent NOT high → harass instead (slot 0).
        assert_eq!(launch_choice(&ars, true, false), Some(0));
    }

    #[test]
    fn launch_never_fires_a_skip_weapon() {
        let mut ars = empty_arsenal();
        ars[0] = idx(WeaponToken::Meadow); // Skip (helps victim)
        ars[1] = idx(WeaponToken::Mirror); // Skip (defensive)
        assert_eq!(
            launch_choice(&ars, true, true),
            None,
            "only Skip weapons → hold fire"
        );
    }

    #[test]
    fn launch_holds_on_empty_arsenal() {
        assert_eq!(launch_choice(&empty_arsenal(), false, false), None);
    }
}
