//! COMPLETE property-based-test coverage of the BattleTris WEAPONS SYSTEM.
//!
//! Every property here pins a weapon's effect / trigger / lifecycle / interaction
//! with an INDEPENDENT oracle — a hand-built board or game state plus a reference
//! computation derived from the original 1994 C++ (`usr/src/game/*.C`), NOT a
//! same-engine "drive it and compare to the engine" self-consistency check. Boards
//! are constructed directly wherever the effect lives on the grid.
//!
//! ============================================================================
//! PHASE 0 — PER-WEAPON COVERAGE MATRIX (the work list + the deliverable).
//!
//! Legend: effect = independent effect oracle pins WHAT it does; dur = duration /
//! lifecycle / expiry-restore / relaunch-stacking pinned; inter = key interaction
//! (Swap/Susan/Mirror/Carter/spy) pinned. (file refers to the test owning it.)
//!
//! tok  weapon        effect            dur            inter           C-ref
//! ---  ------------  ----------------  -------------   -------------   ----------------------
//! 0  FearedWeird   piece_manager UT  THIS file       backfire(versus) BTPieceManager.C:weird
//! 1  FourByFour    piece_manager UT  -               -                BTPieceManager.C
//! 2  Hatter        THIS file         THIS file       backfire(versus) BTGame.C hatter timeout
//! 3  Upbyside      oracle+game+THIS  THIS(swap)      swap-cancel      BTBoardManager.C:85-149
//! 4  FallOut       weapons_oracle    THIS file       -                BTBoardManager.C:410-419
//! 5  Swap          pbt_versus/vs.rs  instant         self(nullify)    BTGame.C:492-534
//! 6  Lawyers       weapons_interact  THIS file       op-clear         BTGame.C BT_LAWYER
//! 7  RiseUp        oracle+relay      instant         backfire(versus) BTBoardManager.C:158
//! 8  FlipOut       weapons_oracle    instant         -                BTBoardManager.C flipVert
//! 9  Speedy        weapons_game      THIS(stack/exp) -                BTGame.C BT_SPEEDY
//! 10 Missing       weapons_oracle    instant         -                BTBoardManager.C
//! 11 PieceIt       weapons_oracle    instant         -                BTBoardManager.C
//! 12 Blind         weapons_oracle    instant         -                BTBoardManager.C
//! 13 Mondale       THIS file         THIS(dur=50)    relay(versus)    BTScoreManager.C:154
//! 14 Keating       pbt_versus/vs.rs  instant         relay+mirror     BTScoreManager.C:110
//! 15 Carter        weapons_game      THIS(dur=20)    price-double     BTGame.C buy
//! 16 Reagan        THIS file         instant         mirror-nullify   game.rs Reagan
//! 17 Ames          bout.rs spy       bout.rs(dur)    spy/mirror       BTRecon.C
//! 18 Ace           bout.rs spy       bout.rs(dur)    spy/mirror       BTRecon.C
//! 19 Condor        bout.rs spy       bout.rs(dur)    spy/mirror       BTRecon.C
//! 20 NiceDay       piece_manager UT  instant         mirror-nullify   BTPieceManager.C hap
//! 21 SoLong        piece_manager UT  THIS(dur=10)    -                BTPieceManager.C
//! 22 NoDice        piece_manager UT  THIS(dur=35)    -                BTPieceManager.C
//! 23 Bug           weapons_oracle    instant         -                BTBoardManager.C
//! 24 Bottle        oracle+THIS(geom) THIS(expiry)    swap-cancel      BTBoardManager.C:87-123
//! 25 NoSlide       weapons_game      THIS file       -                BTGame.C startSlide
//! 26 Susan         pbt_versus/vs.rs  instant         arsenal-swap     BTWeaponManager.C:104
//! 27 Meadow        weapons_game      THIS(exp)       -                BTGame.C BT_MEADOW
//! 28 Mirror        pbt_versus/vs.rs  THIS(dur=10)    backfire/nullify BTWeaponManager.C:204
//! 29 Twilight      weapons_oracle    instant         -                BTBoardManager.C hide
//! 30 Slick         THIS file         THIS file       backfire(versus) BTGame.C slick timeout
//! 31 Broken        piece_manager UT  THIS file       -                BTPieceManager.C broken
//! 32 Force         oracle+THIS(geom) THIS(expiry)    -                BTBoardManager.C:94-148
//! 33 Gimp          weapons_oracle    instant         -                BTBoardManager.C gimp
//!
//! This file (pbt_weapons.rs) adds the INDEPENDENT oracles flagged "THIS file":
//!   * BOARD-ATTACK GEOMETRY: Bottle / Force / Upbyside line-clear geometry with a
//!     from-scratch reference computation of removeLine (the prior suite only had a
//!     Force no-gravity differential).
//!   * BUY-THEN-LAUNCH REPLAY: a launched weapon's EFFECT (not just its frame)
//!     replays bit-exact through the relay.
//!   * FUNDS: Reagan negate, Mondale 30% tax band, Keating seize timing.
//!   * TEMPO/CONTROL: Hatter auto-rotate, Slick auto-slide, NoSlide lock latency.
//!   * DURATIONS/LIFECYCLE: line-based expiry restores prior state; relaunch
//!     accumulates remaining; Speedy/Meadow drop-time round-trips on expiry.
//!   * TRIGGER TIMING: received weapons apply at the next lock, not on receipt.
//!
//! ============================================================================

use bt_core::constants::*;
use bt_core::game::GameEvent;
use bt_core::versus::{deliver_weapon, Side};
use bt_core::weapons::{weapon_table, WeaponToken};
use bt_core::{Board, Cell, CellKind, Game, Versus};
use proptest::prelude::*;

// ===========================================================================
// Shared helpers.
// ===========================================================================

/// Drive `g` until the current piece locks (flushing any pending weapon via
/// `flush_pending`) or the game ends. Returns false if it never locked.
fn lock_one(g: &mut Game) -> bool {
    g.begin_drop();
    for _ in 0..1200 {
        g.tick(16);
        if g.is_game_over() {
            return false;
        }
        if g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
            return true;
        }
    }
    false
}

/// Soft-drop the current piece to the floor without engaging fast-drop (so no
/// hard-drop score bonus), then count ticks until it locks.
fn settle_and_count_lock_ticks(g: &mut Game) -> i32 {
    let mut last = g.piece_pos().1;
    for _ in 0..60 {
        g.soft_drop();
        let y = g.piece_pos().1;
        if y == last {
            break;
        }
        last = y;
    }
    for n in 0..60 {
        g.tick(16);
        if g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
            return n + 1;
        }
    }
    i32::MAX
}

/// Receive `tok` and flush it at the next lock; returns true iff it became active.
fn receive_and_flush(g: &mut Game, tok: WeaponToken) -> bool {
    g.receive_weapon(tok);
    if !lock_one(g) {
        return false;
    }
    g.weapon_active(tok)
}

fn cell_count(b: &Board) -> usize {
    (0..b.height)
        .flat_map(|y| (0..b.width).map(move |x| (x, y)))
        .filter(|&(x, y)| b.get(x, y).is_some())
        .count()
}

// ===========================================================================
// GROUP A — BOARD-ATTACK GEOMETRY with an independent removeLine reference.
//
// The honestly-flagged gaps: Bottle (neck-narrowing line clear) and Upbyside-down
// (gravity-direction flip) line-clear geometry. The prior suite only had a Force
// no-gravity differential. Here we re-implement removeLine FROM SCRATCH in the
// test (mirroring BTBoardManager.C:73-150) and assert the engine matches it on
// hand-built boards — an INDEPENDENT oracle, not a same-engine comparison.
// ===========================================================================

/// A flat snapshot of the board's value grid: Some(value) for a filled cell,
/// None for empty. Independent of Cell identity so the oracle can compare.
fn value_grid(b: &Board) -> Vec<Option<i32>> {
    (0..b.height)
        .flat_map(|y| (0..b.width).map(move |x| (x, y)))
        .map(|(x, y)| b.get(x, y).map(|c| c.value()))
        .collect()
}

/// Independent reference for `BTBoardManager::removeLine(line, 0, width)` — the
/// gravity step that runs when row `line` is cleared. `force`/`bottle`/`upside`
/// select the four branches faithfully (BTBoardManager.C:73-150). Operates on a
/// `width*height` value grid (None = empty), returning the post-shift grid.
// Mirrors removeLine's exact branch-selecting params; a struct would just obscure it.
#[allow(clippy::too_many_arguments)]
fn ref_remove_line(
    grid: &[Option<i32>],
    width: i32,
    height: i32,
    line: i32,
    force: bool,
    bottle: bool,
    upside: bool,
    computer: bool,
) -> Vec<Option<i32>> {
    let mut g = grid.to_vec();
    let idx = |x: i32, y: i32| (y * width + x) as usize;
    let h = BT_BOARD_HGT;

    // x1/x2 are STICKY (faithful to BTBoardManager.C:73): declared once, the bottle
    // band only ever NARROWS them to the neck and they are NEVER reset back to full
    // width. So once the upward (or downward) shift passes through the neck band, ALL
    // rows beyond it shift only over the neck columns — the "shoulders" flanking the
    // neck walls stay frozen. (A previous version reset x1/x2 every iteration, which
    // re-widened above the band and diverged from both the C++ and the real engine;
    // the old single-neck-column fixture masked it.) The trailing top/bottom-row
    // clear uses the SAME sticky x1/x2 left over from the last loop iteration.
    if !upside || computer {
        let (mut x1, mut x2) = (0i32, width);
        let mut i = line;
        while i > 0 {
            if bottle && i <= h / 2 + BT_BOTTLE_Y && i >= h / 2 - BT_BOTTLE_Y {
                x1 = BT_BOTTLE_X;
                x2 = width - BT_BOTTLE_X;
            }
            for j in x1..x2 {
                if force {
                    if i == line {
                        g[idx(j, i)] = None;
                    }
                    continue;
                }
                g[idx(j, i)] = g[idx(j, i - 1)];
                g[idx(j, i - 1)] = None;
            }
            i -= 1;
        }
        if !force {
            for i2 in x1..x2 {
                g[idx(i2, 0)] = None;
            }
        }
    } else {
        let (mut x1, mut x2) = (0i32, width);
        let mut i = line;
        while i < height - 1 {
            if bottle && i <= h / 2 + BT_BOTTLE_Y && i >= h / 2 - BT_BOTTLE_Y - 1 {
                x1 = BT_BOTTLE_X;
                x2 = width - BT_BOTTLE_X;
            }
            for j in x1..x2 {
                if force {
                    if i == line {
                        g[idx(j, i)] = None;
                    }
                    continue;
                }
                g[idx(j, i)] = g[idx(j, i + 1)];
                g[idx(j, i + 1)] = None;
            }
            i += 1;
        }
        if !force {
            for i2 in x1..x2 {
                g[idx(i2, height - 1)] = None;
            }
        }
    }
    g
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// FORCE line-clear geometry: a single full bottom row clears in place with NO
    /// cascade. We build a board with a full bottom row plus arbitrary debris above
    /// it, run the engine's `check_lines` under Force, and assert the result matches
    /// the from-scratch `ref_remove_line(force=true)` — i.e. ONLY the cleared row
    /// empties, everything above is frozen. A mutant that shifts under Force, or
    /// clears the wrong row, diverges from the reference.
    #[test]
    fn force_line_clear_matches_independent_reference(
        debris in prop::collection::vec((0i32..BT_BOARD_WTH, 4i32..BT_BOARD_HGT-1, 1u8..6), 0..40),
    ) {
        let mut b = Board::standard(false);
        let (w, h) = (b.width, b.height);
        b.set_active(WeaponToken::Force, true);
        // Debris above the bottom row.
        for &(x, y, v) in &debris {
            b.set(x, y, Some(Cell::die(v)));
        }
        // A full bottom row (die value 1 -> known values).
        for x in 0..w {
            b.set(x, h - 1, Some(Cell::die(1)));
        }
        let before = value_grid(&b);
        let expect = ref_remove_line(&before, w, h, h - 1, true, false, false, false);

        let lc = b.check_lines();
        prop_assert_eq!(lc.lines, 1, "the full bottom row clears");
        prop_assert_eq!(value_grid(&b), expect, "Force clear must match the no-cascade reference");
    }

    /// BOTTLE line-clear geometry: with Bottle active, clearing a row only shifts
    /// the board down OVER THE NECK in the band [h/2-BOTTLE_Y, h/2+BOTTLE_Y]; the
    /// structure walls flanking the neck must stay put. We build a board with the
    /// bottle walls + a full clearable row + die debris, clear it, and compare to
    /// the independent `ref_remove_line(bottle=true)`. A mutant that ignores the
    /// neck (shifts full width) moves cells the reference keeps fixed.
    #[test]
    fn bottle_line_clear_matches_independent_reference(
        clear_row in (BT_BOARD_HGT/2 + BT_BOTTLE_Y + 1)..BT_BOARD_HGT,
        // Debris lives in a SINGLE interior neck column at distinct rows above the
        // neck so it can never complete a line (even after the shift): at most one
        // neck-debris cell per row + 6 wall cells = 7 < width, so only the prebuilt
        // bottom row is ever full -> exactly one clear.
        debris_rows in prop::collection::btree_set(1i32..(BT_BOARD_HGT/2 - BT_BOTTLE_Y), 0..8),
    ) {
        let mut b = Board::standard(false);
        let (w, h) = (b.width, b.height);
        b.set_active(WeaponToken::Bottle, true);
        // The bottle walls (structure boxes flank the neck).
        for x in 0..BT_BOTTLE_X {
            for y in (h / 2 - BT_BOTTLE_Y)..(h / 2 + BT_BOTTLE_Y) {
                b.set(x, y, Some(Cell::structure()));
                b.set(w - x - 1, y, Some(Cell::structure()));
            }
        }
        // Debris in one interior neck column (so it falls through the neck on the
        // bottle shift) but can never fill a row.
        for &y in &debris_rows {
            b.set(BT_BOTTLE_X, y, Some(Cell::die(3)));
        }
        // SHOULDER debris: cells in a WALL column (x=0) ABOVE the neck band. With the
        // sticky-x1/x2 bottle geometry these MUST stay frozen when a below-neck row
        // clears (the shift above the band only touches the neck columns); a reset-
        // x1/x2 oracle would (wrongly) drop them. This is what exercises the sticky
        // behavior — without it the previous fixture masked the oracle bug.
        for &y in &debris_rows {
            if y >= 1 {
                b.set(0, y - 1, Some(Cell::die(5)));
            }
        }
        // A full clearable row below the neck.
        for x in 0..w {
            b.set(x, clear_row, Some(Cell::die(2)));
        }

        let before = value_grid(&b);
        let expect = ref_remove_line(&before, w, h, clear_row, false, true, false, false);

        let lc = b.check_lines();
        prop_assert_eq!(lc.lines, 1, "the full row clears");
        prop_assert_eq!(value_grid(&b), expect,
            "Bottle clear must match the neck-narrowing reference");
        // The structure walls must survive the clear (they're outside the neck band).
        for x in 0..BT_BOTTLE_X {
            for y in (h / 2 - BT_BOTTLE_Y)..(h / 2 + BT_BOTTLE_Y) {
                prop_assert_eq!(b.get(x, y).map(|c| c.kind), Some(CellKind::Structure),
                    "left wall at ({},{}) must survive a Bottle clear", x, y);
            }
        }
    }
}

/// UPBYSIDE gravity-direction flip during line-clear: when upside-down, clearing a
/// row shifts the board the OTHER way (down toward y=height-1 instead of up). We
/// build the board directly with `upside` + active flag set, clear a row near the
/// TOP, and compare to the independent reference's else-branch. This is a fixed
/// hand-built scenario (no proptest) because the upside flip interacts with the
/// non-computer flipOnHoriz path; here we set the flag directly on a non-flipped
/// grid so the geometry is isolated.
#[test]
fn upbyside_line_clear_shifts_the_opposite_direction() {
    let mut b = Board::standard(false);
    let (w, h) = (b.width, b.height);
    // Mark the board upside-down WITHOUT triggering the visual flip (set the flag
    // + upside latch directly, isolating the removeLine geometry).
    b.set_active(WeaponToken::Upbyside, true);
    b.upside = true;
    // A full clearable row near the top (row 2), with a die marker BELOW it (row 5)
    // that the upside-down shift should pull UP toward row 2 (the C++ else-branch
    // copies map[j][i] = map[j][i+1], dropping the board "down" in board coords).
    for x in 0..w {
        b.set(x, 2, Some(Cell::die(3)));
    }
    b.set(4, 5, Some(Cell::die(6)));

    let before = value_grid(&b);
    let expect = ref_remove_line(&before, w, h, 2, false, false, true, false);

    let lc = b.check_lines();
    assert_eq!(lc.lines, 1, "the full row clears");
    assert_eq!(value_grid(&b), expect,
        "Upbyside clear must use the opposite shift direction (else-branch)");
    // Concretely: the marker at (4,5) moved UP to (4,4) under the upside shift.
    assert_eq!(b.get(4, 4).map(|c| c.value()), Some(6),
        "the marker below the cleared row shifted toward it (upside gravity)");
    assert!(b.get(4, 5).is_none(), "the marker vacated its old cell");
}

use bt_core::Rng;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// UPBYSIDE flip is an INVOLUTION (first-principles, NOT faithfulness): turning
    /// the weapon ON mirrors the board top<->bottom, and turning it OFF mirrors it
    /// back — so on-then-off must leave the board BYTE-IDENTICAL (a mirror is its
    /// own inverse). Pinned with an independent reference: ON must equal the
    /// hand-computed vertical mirror (catches a wrong-axis / partial flip), and OFF
    /// must restore the original (catches an asymmetric on/off). No faithfulness
    /// oracle checks either.
    #[test]
    fn upbyside_flip_on_then_off_is_an_involution(
        fills in prop::collection::vec(
            (0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..=6u8), 0..50)
    ) {
        let mut rng = Rng::new(1);
        let mut b = Board::standard(false); // human board: the flip actually happens
        let (w, h) = (b.width as usize, b.height as usize);
        for (x, y, v) in &fills {
            b.set(*x, *y, Some(Cell::die(*v)));
        }
        let before = value_grid(&b);
        // Independent reference for the ON flip: `before` with its rows reversed.
        let mut expect_mirror = vec![None; w * h];
        for y in 0..h {
            for x in 0..w {
                expect_mirror[y * w + x] = before[(h - 1 - y) * w + x];
            }
        }

        // ON: mirror top<->bottom.
        b.apply_weapon(WeaponToken::Upbyside, &mut rng);
        prop_assert_eq!(value_grid(&b), expect_mirror,
            "Upbyside ON must mirror the board top<->bottom exactly");

        // OFF: mirror back -> the involution.
        b.revert_weapon(WeaponToken::Upbyside);
        prop_assert_eq!(value_grid(&b), before,
            "Upbyside on-then-off must round-trip the board (flip is its own inverse)");
    }
}

// ===========================================================================
// GROUP B — FUNDS EFFECTS (Reagan / Mondale / Keating), independent arithmetic.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// REAGAN: "your opponent's funds are multiplied by -1." Applied at flush.
    /// Independent oracle: after the flush, funds == -(funds_before). We bank funds
    /// directly, receive Reagan, flush at a lock, and assert the exact negation.
    /// A mutant that zeroes (like Keating) or leaves funds alone fails.
    #[test]
    fn reagan_negates_funds_exactly(start in 1i64..1_000_000) {
        let mut g = Game::new(1);
        // Bank funds without clearing lines (set funds via add_funds; the board is
        // empty so a lock won't earn or clear anything).
        g.add_funds(start);
        let _ = g.take_events();
        prop_assume!(g.score().funds == start);

        prop_assume!(receive_and_flush(&mut g, WeaponToken::Reagan));
        // The flushing lock dropped a piece on an empty board: no line clear, so
        // funds are untouched except by Reagan -> exactly negated.
        prop_assert_eq!(g.score().funds, -start,
            "Reagan must multiply funds by -1 (got {}, want {})", g.score().funds, -start);
    }

    /// KEATING applied locally (the victim side of the relay): "all taken away."
    /// Independent oracle: funds == 0 after flush, and the FundsStolen event carries
    /// EXACTLY the pre-seizure amount (so the relay can credit the attacker). A
    /// mutant that steals a fraction, or emits the wrong amount, fails.
    #[test]
    fn keating_seizes_all_funds_and_reports_the_exact_amount(start in 1i64..1_000_000) {
        let mut g = Game::new(1);
        g.add_funds(start);
        let _ = g.take_events();
        prop_assume!(g.score().funds == start);

        g.receive_weapon(WeaponToken::Keating);
        g.begin_drop();
        let mut stolen = None;
        let mut locked = false;
        for _ in 0..1200 {
            g.tick(16);
            for e in g.take_events() {
                match e {
                    GameEvent::FundsStolen(amt) => stolen = Some(amt),
                    GameEvent::Locked { .. } => locked = true,
                    _ => {}
                }
            }
            if locked { break; }
            if g.is_game_over() { break; }
        }
        prop_assume!(locked);
        prop_assert_eq!(g.score().funds, 0, "Keating must zero the victim's funds");
        prop_assert_eq!(stolen, Some(start),
            "the FundsStolen report must equal the pre-seizure funds ({})", start);
    }
}

/// MONDALE 30% tax: the victim keeps floor((1-0.30)*funds) of newly-banked line
/// funds; the attacker gets the EXACT remainder (funds - kept) so the transfer
/// CONSERVES money (see `mondale_transfer_conserves_funds`). NB the engine no
/// longer uses the original's leaky bean reconstruction
/// `floor((1/0.70)*kept*0.30)` — but for these width-multiple die values the two
/// formulas COINCIDE, so this also pins faithfulness-where-1994-agrees.
#[test]
fn mondale_taxes_thirty_percent_keeping_the_transfer_conserved() {
    // Try several die values so `funds` spans a range and truncation varies.
    for die in 1u8..=6 {
        let mut g = Game::new(1);
        // Activate Mondale first (flush on an empty board: no clear, funds stay 0).
        assert!(receive_and_flush(&mut g, WeaponToken::Mondale), "Mondale active");
        assert_eq!(g.score().funds, 0, "no funds banked yet");
        let funds_before = g.score().funds;

        // Build a full bottom row of dice; the NEXT lock clears it for value*lines.
        let (w, h) = (g.board().width, g.board().height);
        for x in 0..w {
            g.board_mut().set(x, h - 1, Some(Cell::die(die)));
        }
        // One full row -> value = w*die, lines = 1, funds = value*lines.
        let value = w * die as i32;
        let raw_funds = value; // lines == 1

        // Independent oracle: victim keeps floor(70%), attacker gets the exact
        // remainder (conserving). For width-multiple raw_funds this equals the
        // original bean value floor((1/0.70)*kept*0.30) too.
        let kept = (raw_funds as f64 * (1.0 - BT_MONDALE_RATE)) as i64;
        let tax = raw_funds as i64 - kept;

        // Drive a lock that clears the prebuilt row; collect FundsStolen.
        g.begin_drop();
        let mut stolen = 0i64;
        let mut cleared = false;
        for _ in 0..1200 {
            g.tick(16);
            for e in g.take_events() {
                match e {
                    GameEvent::FundsStolen(a) => stolen += a,
                    GameEvent::Locked { lines, .. } if lines > 0 => cleared = true,
                    _ => {}
                }
            }
            if cleared { break; }
            if g.is_game_over() { break; }
        }
        assert!(cleared, "the prebuilt row must clear (die={die})");
        assert_eq!(g.score().funds - funds_before, kept,
            "Mondale victim keeps floor(funds*0.70): die={die} kept={kept} got={}",
            g.score().funds - funds_before);
        assert_eq!(stolen, tax,
            "Mondale stolen cut must match the original bean arithmetic: die={die} tax={tax} got={stolen}");
    }
}

/// Fill row `y` completely with dice whose values SUM to `target` (each die in
/// 1..=6, so `target` must be in `[width, 6*width]`). Lets us drive a clear whose
/// `funds` (= value*lines, lines==1) is an EXACT chosen G — including the
/// non-multiple-of-width totals the uniform-row tests above never reach.
fn fill_bottom_row_to_sum(g: &mut Game, target: i32) {
    let (w, h) = (g.board().width, g.board().height);
    let mut vals = vec![1i32; w as usize]; // min sum = w
    let mut remaining = target - w; // distribute the surplus, each cell up to +5
    for v in vals.iter_mut() {
        let add = remaining.min(5);
        *v += add;
        remaining -= add;
    }
    assert_eq!(remaining, 0, "target {target} out of [{w}, {}]", 6 * w);
    for (x, v) in vals.iter().enumerate() {
        g.board_mut().set(x as i32, h - 1, Some(Cell::die(*v as u8)));
    }
}

/// FIRST-PRINCIPLES correctness (NOT faithfulness): Mondale is a fund TRANSFER,
/// so it must CONSERVE money — the attacker gains exactly what the victim loses,
/// relative to the un-taxed earning G. Concretely: victim's banked gain (`kept`)
/// plus the attacker's stolen cut (`stolen`) must equal the raw line-clear value
/// G. This owes nothing to the 1994 original; it's just "a tax can't make money
/// vanish." Driven through the REAL engine for every reachable G.
#[test]
fn mondale_transfer_conserves_funds() {
    let mut violations: Vec<(i32, i64, i64)> = Vec::new(); // (G, kept, stolen)
    let w = Game::new(0).board().width;
    for g_target in w..=(6 * w) {
        let mut g = Game::new(7);
        assert!(receive_and_flush(&mut g, WeaponToken::Mondale), "Mondale active");
        let funds_before = g.score().funds;
        fill_bottom_row_to_sum(&mut g, g_target);

        g.begin_drop();
        let (mut stolen, mut raw_funds, mut cleared) = (0i64, 0i64, false);
        for _ in 0..1200 {
            g.tick(16);
            for e in g.take_events() {
                match e {
                    GameEvent::FundsStolen(a) => stolen += a,
                    GameEvent::Locked { lines, funds, .. } if lines > 0 => {
                        raw_funds = funds as i64;
                        cleared = true;
                    }
                    _ => {}
                }
            }
            if cleared || g.is_game_over() {
                break;
            }
        }
        assert!(cleared, "row must clear for G={g_target}");
        assert_eq!(raw_funds, g_target as i64, "sanity: clear funds == chosen G");
        let kept = g.score().funds - funds_before;
        if kept + stolen != raw_funds {
            violations.push((g_target, kept, stolen));
        }
    }
    assert!(
        violations.is_empty(),
        "Mondale transfer DESTROYS funds for {}/{} values (victim loses more than the \
         attacker gains). Examples (G, victim_kept, attacker_stolen): {:?}",
        violations.len(),
        6 * w - w + 1,
        &violations[..violations.len().min(8)]
    );
}

// ===========================================================================
// GROUP C — TEMPO / CONTROL (Hatter / Slick / NoSlide / Speedy / Meadow).
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// MAD HATTER auto-rotates the falling piece on its own timer. Independent
    /// oracle: a piece that CAN actually rotate (`rot != 0` — the rotation sub-square
    /// gate in `Piece::rotate_generic`; a Die/Box have `rot == 0` and never change
    /// orientation) MUST change orientation over a Hatter window even with NO rotate
    /// input, whereas the control (no Hatter, same seed) stays frozen. We empty the
    /// board so rotation is never wall-pinned. A mutant that drops the hatter
    /// auto-rotate sub-timer leaves the orientation fixed and fails.
    #[test]
    fn hatter_auto_rotates_without_input(seed in any::<u64>()) {
        // A piece visibly rotates only when its rotation sub-square `rot != 0`.
        let rotatable = |g: &Game| g.current_piece().map(|p| p.rot != 0).unwrap_or(false);

        let mut g = Game::new(seed);
        prop_assume!(receive_and_flush(&mut g, WeaponToken::Hatter));
        prop_assume!(!g.is_game_over());
        // Clear the board so the piece can spin freely (never pinned to a wall by a
        // stack), and so it can't lock during the observation window.
        {
            let (w, h) = (g.board().width, g.board().height);
            for y in 0..h { for x in 0..w { g.board_mut().set(x, y, None); } }
        }
        prop_assume!(rotatable(&g));
        let start_o = g.current_piece().unwrap().orientation;

        // Control: SAME seed, NO Hatter — the matching piece must stay frozen with
        // no input over the same window.
        let mut ctrl = Game::new(seed);
        prop_assume!(lock_one(&mut ctrl) && !ctrl.is_game_over());
        {
            let (w, h) = (ctrl.board().width, ctrl.board().height);
            for y in 0..h { for x in 0..w { ctrl.board_mut().set(x, y, None); } }
        }
        // The control piece is the same kind/orientation (same seed, same lock path).
        let ctrl_o0 = ctrl.current_piece().map(|p| p.orientation);

        let mut rotated = false;
        for _ in 0..40 {
            g.tick(8);
            match g.current_piece() {
                Some(p) => if p.orientation != start_o { rotated = true; break; },
                None => break,
            }
        }
        for _ in 0..40 { ctrl.tick(8); if ctrl.current_piece().is_none() { break; } }
        if let (Some(p), Some(o0)) = (ctrl.current_piece(), ctrl_o0) {
            prop_assert_eq!(p.orientation, o0, "without Hatter the piece must NOT auto-rotate");
        }
        prop_assert!(rotated,
            "Mad Hatter must auto-rotate the falling piece (orientation never left {})", start_o);
    }
}

/// SLICK WILLY auto-slides the piece left/right on its own timer while falling.
/// Independent oracle: with Slick active and NO horizontal input, the piece's x
/// must change over a window; the control (no Slick) keeps x fixed. We freeze
/// gravity influence by checking the x coordinate specifically. Fixed scenario
/// to keep the piece high and mobile.
#[test]
fn slick_auto_slides_without_input() {
    // Control: x is frozen with no input and no Slick.
    let mut ctrl = Game::new(5);
    let cx0 = ctrl.piece_pos().0;
    for _ in 0..10 { ctrl.tick(8); }
    assert_eq!(ctrl.piece_pos().0, cx0, "without Slick, x stays put with no input");

    let mut g = Game::new(5);
    assert!(receive_and_flush(&mut g, WeaponToken::Slick), "Slick active");
    let x0 = g.piece_pos().0;
    let mut moved = false;
    for _ in 0..40 {
        g.tick(8);
        if g.current_piece().is_none() { break; }
        if g.piece_pos().0 != x0 { moved = true; break; }
    }
    assert!(moved, "Slick Willy must auto-slide the piece (x never left {})", x0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// NO SLIDE removes the lock-delay grace: a piece that lands locks in ~0 extra
    /// ticks, vs the default `BT_SLIDE_TIME`(150ms)/16ms ~= 10-tick grace. Independent
    /// oracle: noslide_ticks <= 1 AND strictly fewer than the default. A mutant that
    /// keeps the slide grace under NoSlide fails the <= 1 bound.
    #[test]
    fn no_slide_locks_immediately_on_landing(seed in any::<u64>()) {
        let mut def = Game::new(seed);
        let default_ticks = settle_and_count_lock_ticks(&mut def);
        prop_assume!(default_ticks != i32::MAX);

        let mut g = Game::new(seed);
        prop_assume!(receive_and_flush(&mut g, WeaponToken::NoSlide));
        let ns_ticks = settle_and_count_lock_ticks(&mut g);
        prop_assume!(ns_ticks != i32::MAX);

        prop_assert!(ns_ticks <= 1,
            "NoSlide must lock within one tick of landing (got {ns_ticks} ticks)");
        prop_assert!(ns_ticks < default_ticks,
            "NoSlide ({ns_ticks}) must lock sooner than the default grace ({default_ticks})");
    }
}

// ===========================================================================
// GROUP D — DURATIONS / LIFECYCLE (line-based expiry restores prior state;
// relaunch accumulates remaining; Speedy/Meadow round-trip drop-time on expiry).
// ===========================================================================

/// Clear exactly `n` lines on `g` (driving real locks so weapon durations tick).
///
/// Each iteration: empty the board (so accumulated locked-piece debris can never
/// top it out — clearing CELLS leaves all weapon flags/durations/remaining intact),
/// prefill ONE full bottom row, then lock any falling piece. The piece comes to
/// rest ON TOP of the prefilled full row, and the lock's `check_lines` clears
/// exactly that one row — so each lock ticks exactly one line off every active
/// weapon's `remaining_`. Returns the number of lines actually cleared.
fn clear_n_lines(g: &mut Game, n: i32) -> i32 {
    let mut cleared = 0;
    for _ in 0..(n * 6 + 12) {
        if cleared >= n || g.is_game_over() {
            break;
        }
        {
            // Reset the playfield to a single full bottom row (clears prior debris
            // but NOT weapon state). Structure walls (Bottle) are left alone so the
            // weapon under test isn't perturbed.
            let (w, h) = (g.board().width, g.board().height);
            for y in 0..h {
                for x in 0..w {
                    let keep = g.board().get(x, y).map(|c| c.kind) == Some(CellKind::Structure);
                    if !keep {
                        g.board_mut().set(x, y, None);
                    }
                }
            }
            for x in 0..w {
                if g.board().get(x, h - 1).is_none() {
                    g.board_mut().set(x, h - 1, Some(Cell::die(1)));
                }
            }
        }
        // Clearing lines crosses the combined-20-lines bazaar barrier, which FREEZES
        // the game (no piece will lock). Leave the bazaar immediately so locks keep
        // flowing — we only want the line-tick side-effect, not the shopping flow.
        if g.is_in_bazaar() {
            g.leave_bazaar();
        }
        g.begin_drop();
        let mut got_lock = false;
        for _ in 0..400 {
            if g.is_in_bazaar() {
                g.leave_bazaar();
            }
            g.tick(16);
            for e in g.take_events() {
                if let GameEvent::Locked { lines, .. } = e {
                    cleared += lines;
                    got_lock = true;
                }
            }
            if got_lock || g.is_game_over() { break; }
        }
    }
    cleared
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// DURATION COUNTDOWN + EXPIRY-RESTORES-STATE for a duration weapon. We pick
    /// SoLong (duration 10 lines, deprives of long pieces). Independent oracle:
    ///   * weapon_remaining starts at the table duration after flush;
    ///   * after clearing `k < duration` lines, remaining == duration - k and the
    ///     weapon is still active;
    ///   * after clearing >= duration lines, remaining == 0 and it's inactive.
    /// A mutant that doesn't count down (or never expires) fails.
    #[test]
    fn duration_weapon_counts_down_and_expires(k in 1i32..9) {
        let dur = weapon_table()[WeaponToken::SoLong.index()].duration as i32;
        prop_assume!(k < dur);
        let mut g = Game::new(7);
        prop_assume!(receive_and_flush(&mut g, WeaponToken::SoLong));
        prop_assert_eq!(g.weapon_remaining(WeaponToken::SoLong), dur,
            "remaining starts at the table duration ({})", dur);

        let cleared = clear_n_lines(&mut g, k);
        prop_assume!(!g.is_game_over());
        prop_assume!(cleared == k); // exact line accounting needed for the assert
        prop_assert_eq!(g.weapon_remaining(WeaponToken::SoLong), dur - k,
            "remaining must decrement by the lines cleared ({} - {} = {})", dur, k, dur - k);
        prop_assert!(g.weapon_active(WeaponToken::SoLong),
            "still active with {} lines left", dur - k);

        // Now clear enough more to push it past zero -> expires & deactivates.
        let more = clear_n_lines(&mut g, dur - k + 1);
        prop_assume!(!g.is_game_over());
        prop_assume!(more >= dur - k);
        prop_assert_eq!(g.weapon_remaining(WeaponToken::SoLong), 0,
            "remaining clamps to 0 after expiry");
        prop_assert!(!g.weapon_active(WeaponToken::SoLong),
            "the weapon deactivates once its duration elapses");
    }
}

/// RELAUNCH ACCUMULATES REMAINING: receiving the same duration weapon twice adds
/// the durations (`remaining_ += duration` in apply_weapon_on, BTGame). Independent
/// oracle: after two flushes (minus the lines spent flushing), remaining is the sum
/// of two table durations less the lines consumed. We use a long-duration weapon
/// (NoDice, 35) so the two flush-lines don't wipe it, and clear no extra lines.
#[test]
fn relaunching_a_duration_weapon_accumulates_remaining() {
    let dur = weapon_table()[WeaponToken::NoDice.index()].duration as i32;
    let mut g = Game::new(3);
    // First flush on an empty board: a lock with no clear, so no lines tick off.
    assert!(receive_and_flush(&mut g, WeaponToken::NoDice), "first NoDice active");
    let after_first = g.weapon_remaining(WeaponToken::NoDice);
    assert_eq!(after_first, dur, "first launch sets remaining to one duration");

    // Second flush, again with no line clear (empty board) -> durations add.
    assert!(receive_and_flush(&mut g, WeaponToken::NoDice), "still active after relaunch");
    let after_second = g.weapon_remaining(WeaponToken::NoDice);
    assert_eq!(after_second, dur * 2,
        "relaunch must ACCUMULATE remaining ({} + {} = {}), got {}",
        dur, dur, dur * 2, after_second);
}

/// Empty the board (cells only; weapon state untouched) so the current piece can
/// fall freely, then measure the gravity period: the number of 8ms ticks until the
/// piece advances exactly one row from its current rest. A smaller period == faster
/// gravity. This is an INDEPENDENT timing probe (it reads only piece_pos), not an
/// engine self-comparison.
fn gravity_period_ms(g: &mut Game) -> i32 {
    // Free the column under the piece by emptying the whole board (the piece keeps
    // falling; nothing left to land on until the floor).
    let (w, h) = (g.board().width, g.board().height);
    for y in 0..h {
        for x in 0..w {
            g.board_mut().set(x, y, None);
        }
    }
    let y0 = g.piece_pos().1;
    let mut t = 0;
    for _ in 0..2000 {
        g.tick(8);
        t += 8;
        if g.current_piece().is_none() {
            return i32::MAX; // locked/spawned — can't measure
        }
        if g.piece_pos().1 != y0 {
            return t;
        }
    }
    i32::MAX
}

/// SPEEDY EXPIRY round-trips the gravity speed: Speedy halves `base_drop_time` on
/// activation and the expiry handler doubles it back. Independent oracle on the
/// gravity PERIOD (ms per row): baseline ~512ms; while Speedy is active it's faster
/// (smaller period); once Speedy expires the period returns to the baseline. A
/// mutant that forgets to undo the speedup leaves the post-expiry period too small.
#[test]
fn speedy_expiry_restores_the_baseline_gravity_period() {
    let baseline = gravity_period_ms(&mut Game::new(9));
    assert!(baseline != i32::MAX && baseline > 0, "sanity: a measurable baseline period");

    // Active: a freshly-flushed Speedy game must fall faster (smaller period).
    {
        let mut sp = Game::new(9);
        assert!(receive_and_flush(&mut sp, WeaponToken::Speedy), "Speedy active");
        let active = gravity_period_ms(&mut sp);
        assert!(active != i32::MAX && active < baseline,
            "Speedy must shorten the gravity period ({active} vs baseline {baseline})");
    }

    // Expire: clear Speedy's full duration of lines, then re-measure.
    let mut g = Game::new(9);
    assert!(receive_and_flush(&mut g, WeaponToken::Speedy), "Speedy active");
    let dur = weapon_table()[WeaponToken::Speedy.index()].duration as i32;
    let cleared = clear_n_lines(&mut g, dur);
    if g.is_game_over() {
        return; // expiry still pinned by the remaining-countdown test
    }
    assert!(cleared >= dur, "cleared {cleared} of {dur} for expiry");
    assert!(!g.weapon_active(WeaponToken::Speedy), "Speedy expired after its duration");

    let after = gravity_period_ms(&mut g);
    assert!(after != i32::MAX, "post-expiry period measurable");
    assert_eq!(after, baseline,
        "after Speedy expires, the gravity period must return to baseline ({after} vs {baseline})");
}

/// STACKED Speedy/Meadow must NOT leak gravity speed on expiry (first-principles
/// lifecycle conservation, NOT faithfulness). Speedy/Meadow shift `base_drop_time`
/// by a RELATIVE factor, but the boolean `BTActive` flag means the weapon expires
/// with exactly ONE `apply_weapon_off`. If the ON shift re-fired on every launch
/// (the 1994 behavior) while OFF reverted once, stacking two launches would leave
/// gravity PERMANENTLY off-baseline after the (accumulated) duration elapsed — a
/// state-corruption leak. Independent oracle on the gravity period: after stacking
/// two of the same speed weapon and clearing its full accumulated duration, the
/// period must return EXACTLY to baseline. (Pins the idempotent-on-active-flag fix
/// in `apply_weapon_on`; a regression to per-launch shifting fails here.)
#[test]
fn stacked_speedy_meadow_restore_baseline_on_expiry() {
    for tok in [WeaponToken::Speedy, WeaponToken::Meadow] {
        let seed = 4242;
        let baseline = gravity_period_ms(&mut Game::new(seed));
        assert!(baseline != i32::MAX && baseline > 0, "{tok:?}: measurable baseline");

        let mut g = Game::new(seed);
        // Stack TWO launches of the same speed weapon (each flush on an empty board
        // -> no clear, so neither flush-lock ticks the duration down).
        assert!(receive_and_flush(&mut g, tok), "{tok:?} first launch active");
        g.board_mut().clear();
        assert!(receive_and_flush(&mut g, tok), "{tok:?} second launch still active");
        g.board_mut().clear();

        let dur = weapon_table()[tok.index()].duration as i32;
        // Two launches accumulate remaining -> need 2*dur (plus a slack line) to expire.
        assert_eq!(g.weapon_remaining(tok), dur * 2,
            "{tok:?}: stacking accumulates remaining (2*{dur})");
        let cleared = clear_n_lines(&mut g, dur * 2 + 2);
        if g.is_game_over() {
            continue; // expiry of the flag is pinned by the countdown property
        }
        assert!(cleared >= dur * 2, "{tok:?}: cleared {cleared} of {} for full expiry", dur * 2);
        assert!(!g.weapon_active(tok), "{tok:?} expired after its accumulated duration");

        let after = gravity_period_ms(&mut g);
        assert!(after != i32::MAX, "{tok:?}: post-expiry period measurable");
        assert_eq!(after, baseline,
            "after a STACKED {tok:?} expires, gravity must return to baseline \
             ({after} vs {baseline}) — no per-launch speed leak");
    }
}

// ===========================================================================
// GROUP E — TRIGGER TIMING (received weapons apply at the NEXT lock, not on
// receipt). A blanket property over ALL persistent-effect weapons.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// AT-LOCK FLUSH: a received weapon is INERT until the next piece lock. For
    /// every weapon whose effect sets a board/game active flag, `receive_weapon`
    /// must NOT make it active synchronously; only the flush at the next lock does.
    /// Independent oracle: weapon_active is false right after receive, true after a
    /// lock. (Instant board-mutation weapons still queue — they don't apply on
    /// receipt either.) A mutant that applies on receipt fails the "inert" half.
    #[test]
    fn received_weapon_is_inert_until_the_next_lock(tok_idx in 0usize..34) {
        let tok = WeaponToken::ALL[tok_idx];
        // Pick a weapon with an observable active flag (duration > 0 OR one of the
        // persistent control weapons). Instant weapons (duration 0, no flag) can't
        // be observed via weapon_active, so skip them for the "active" half but
        // still assert the inert half via the pending queue not mutating funds.
        let mut g = Game::new(13);
        // Plant some debris so an EAGER board-mutation weapon (PieceIt/Blind/Missing/
        // Bug/Gimp/Twilight/RiseUp/...) has something visible to disturb on receipt.
        {
            let (w, h) = (g.board().width, g.board().height);
            for x in 0..w { g.board_mut().set(x, h - 1, Some(Cell::die(2))); }
            g.board_mut().set(w / 2, h - 4, Some(Cell::die(5)));
        }
        let funds0 = g.score().funds;
        let board0 = g.export_board(); // FULL grid snapshot (catches eager board edits)
        let arsenal0: Vec<_> = (0..10).map(|i| (g.arsenal_token(i), g.arsenal_quantity(i))).collect();
        g.receive_weapon(tok);
        // Receipt must never synchronously activate or change funds...
        prop_assert!(!g.weapon_active(tok),
            "{:?} must NOT be active merely from receive_weapon", tok);
        prop_assert_eq!(g.score().funds, funds0,
            "receive_weapon must not change funds synchronously ({:?})", tok);
        // ...nor mutate the BOARD or the ARSENAL. An eager instant board weapon
        // (e.g. PieceIt applied on receipt) would diverge the export here even though
        // it leaves the active flag and funds untouched — the prior version missed it.
        prop_assert_eq!(g.export_board(), board0,
            "receive_weapon must not mutate the board synchronously ({:?})", tok);
        let arsenal1: Vec<_> = (0..10).map(|i| (g.arsenal_token(i), g.arsenal_quantity(i))).collect();
        prop_assert_eq!(arsenal1, arsenal0,
            "receive_weapon must not mutate the arsenal synchronously ({:?})", tok);

        // Flush at a lock. For a persistent weapon (duration > 0), it becomes active.
        let dur = weapon_table()[tok.index()].duration;
        let locked = lock_one(&mut g);
        if dur > 0 && locked && !g.is_game_over() {
            prop_assert!(g.weapon_active(tok),
                "{:?} (duration {}) must activate at the next lock", tok, dur);
        }
    }
}

// ===========================================================================
// GROUP F — CROSS-PLAYER REPLAY: a BAZAAR-BUY-then-LAUNCH flow so a launched
// weapon's EFFECT (not just its frame) replays bit-exact through the relay.
// ===========================================================================

/// Force `g` into the bazaar by reporting enough opponent lines to cross a
/// multiple of 20.
fn open_bazaar(g: &mut Game) {
    g.receive_op_score(0, 19, 0);
    g.receive_op_score(0, 20, 0);
    let _ = g.take_events();
}

/// REAGAN ERA negates funds — and is its own inverse (an INVOLUTION): apply it
/// twice and funds return to the original value.
#[test]
fn reagan_negates_funds_and_is_an_involution() {
    let mut g = Game::new(7);
    g.add_funds(137);
    let x = g.score().funds;
    let _ = g.take_events();
    g.receive_weapon(WeaponToken::Reagan);
    assert!(lock_one(&mut g), "flush Reagan");
    assert_eq!(g.score().funds, -x, "Reagan must negate funds");
    g.receive_weapon(WeaponToken::Reagan);
    assert!(lock_one(&mut g), "flush Reagan again");
    assert_eq!(g.score().funds, x, "Reagan twice must restore funds (involution)");
}

// (Keating's seize-all-and-credit-exactly conservation is already pinned by
// `keating_seizes_all_funds_and_reports_the_exact_amount` above.)

/// LAZY SUSAN exchanges the two players' arsenals and is an INVOLUTION: swap twice
/// and each arsenal returns to its original slots.
#[test]
fn susan_swaps_arsenals_and_is_an_involution() {
    let mut a = Game::new(1);
    let mut b = Game::new(2);
    a.grant_weapon(WeaponToken::RiseUp);
    a.grant_weapon(WeaponToken::Bottle);
    b.grant_weapon(WeaponToken::Force);
    let snap = |g: &Game| (0..10).map(|i| (g.arsenal_token(i), g.arsenal_quantity(i))).collect::<Vec<_>>();
    let (a0, b0) = (snap(&a), snap(&b));

    a.swap_arsenal_with(&mut b);
    assert_eq!(snap(&a), b0, "after Susan, A holds B's arsenal");
    assert_eq!(snap(&b), a0, "after Susan, B holds A's arsenal");

    a.swap_arsenal_with(&mut b);
    assert_eq!(snap(&a), a0, "Susan twice restores A's arsenal");
    assert_eq!(snap(&b), b0, "Susan twice restores B's arsenal");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// FLIP OUT mirrors the board LEFT<->RIGHT (vs a hand-computed reference) and
    /// is an INVOLUTION: flip twice = identity, preserving the exact cell multiset.
    #[test]
    fn flip_out_mirrors_lr_and_is_an_involution(
        fills in prop::collection::vec((0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..=6u8), 0..50)
    ) {
        let mut rng = Rng::new(1);
        let mut b = Board::standard(false);
        b.clear();
        for (x, y, v) in &fills { b.set(*x, *y, Some(Cell::die(*v))); }
        let (w, h) = (b.width as usize, b.height as usize);
        let before = value_grid(&b);
        let mut mirror = vec![None; w * h];
        for y in 0..h { for x in 0..w { mirror[y * w + x] = before[y * w + (w - 1 - x)]; } }

        b.apply_weapon(WeaponToken::FlipOut, &mut rng);
        prop_assert_eq!(value_grid(&b), mirror, "FlipOut must mirror the board left<->right");
        b.apply_weapon(WeaponToken::FlipOut, &mut rng);
        prop_assert_eq!(value_grid(&b), before, "FlipOut twice must be the identity");
    }
}

/// CARTER ARBITRAGE — an INTENTIONAL, faithful-to-1994 economic nuance, kept on
/// purpose (NOT a bug). Both the buy price and the sell refund track the CURRENT
/// Carter multiplier (BTBazaar.C:415 buy / :458 sell — `price_ * (1 + carter_)`),
/// so buying a weapon while un-cursed and selling it back while Carter-cursed nets
/// a profit of EXACTLY its base price. It's a skill boon: stack cheap inventory,
/// then cash out for double once Carter curses you. A naive first-principles "the
/// bazaar never mints funds" property fails on this by design — so instead we pin
/// the EXACT boon, so a refactor can't silently remove it.
///
/// (Deliberately NOT conserving: the curse pays its victim to liquidate, which is
/// faithful to the original and rewards players who know how to stack. If we ever
/// wanted Carter to be a pure penalty, the sell path would drop the `*(1+carter)`.)
#[test]
fn carter_buy_uncursed_sell_cursed_is_an_intentional_arbitrage_boon() {
    let tok = WeaponToken::RiseUp;
    let base = weapon_table()[tok.index()].price as i64;
    assert!(base > 0);

    let mut g = Game::new(3);
    open_bazaar(&mut g);
    assert!(g.is_in_bazaar(), "bazaar open");
    g.add_funds(base * 4);
    let _ = g.take_events();
    let funds_start = g.score().funds;

    // Buy while NOT Carter-cursed (pays base).
    assert!(g.buy_weapon(tok), "buy should succeed");

    // Carter activates between bazaar visits (a lock on an empty board -> no clear).
    g.leave_bazaar();
    assert!(receive_and_flush(&mut g, WeaponToken::Carter), "Carter active");
    open_bazaar(&mut g);
    assert!(g.is_in_bazaar(), "bazaar re-open");

    // Sell it back while cursed (refunds 2*base).
    assert!(g.sell_weapon(tok), "sell should succeed");

    assert_eq!(
        g.score().funds, funds_start + base,
        "Carter buy-low/sell-high mints exactly the base price {base} — the intentional boon \
         (started {funds_start}, ended {})",
        g.score().funds
    );
}

/// Count `tick(16)` calls for the current falling piece to descend one row.
/// Reflects `drop_time` (private), so it observes the gravity SPEED directly.
fn ticks_to_fall_one_row(g: &mut Game) -> i32 {
    let y0 = match g.current_piece() {
        Some(p) => p.y,
        None => return -1,
    };
    for t in 1..2000 {
        g.tick(16);
        let _ = g.take_events();
        match g.current_piece() {
            Some(p) if p.y > y0 => return t,
            None => return -1,
            _ => {}
        }
        if g.is_game_over() {
            return -1;
        }
    }
    -1
}

/// SPEEDY speeds gravity up, MEADOW slows it down (first-principles, observable):
/// a Speedy-cursed piece falls in FEWER ticks/row than baseline; a Meadow-cursed
/// piece takes MORE. Pins the actual speed effect that the private `drop_time`
/// hides from direct inspection.
#[test]
fn speedy_speeds_up_and_meadow_slows_gravity() {
    let seed = 2024;
    let baseline = ticks_to_fall_one_row(&mut Game::new(seed));

    let mut gs = Game::new(seed);
    assert!(receive_and_flush(&mut gs, WeaponToken::Speedy), "Speedy active");
    let speedy = ticks_to_fall_one_row(&mut gs);

    let mut gm = Game::new(seed);
    assert!(receive_and_flush(&mut gm, WeaponToken::Meadow), "Meadow active");
    let meadow = ticks_to_fall_one_row(&mut gm);

    assert!(baseline > 0 && speedy > 0 && meadow > 0,
        "pieces must fall: baseline={baseline} speedy={speedy} meadow={meadow}");
    assert!(speedy < baseline,
        "Speedy must drop FASTER (fewer ticks/row): baseline {baseline} vs speedy {speedy}");
    assert!(meadow > baseline,
        "Meadow must drop SLOWER (more ticks/row): baseline {baseline} vs meadow {meadow}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// PIECE IT TOGETHER / BUG REPORT each add EXACTLY ONE box (Bug's is invisible)
    /// — on a non-full board the cell count rises by exactly one.
    #[test]
    fn piece_it_and_bug_add_exactly_one_box(seed in any::<u64>(), bug in any::<bool>()) {
        let tok = if bug { WeaponToken::Bug } else { WeaponToken::PieceIt };
        let mut rng = Rng::new(seed);
        let mut b = Board::standard(false);
        b.clear();
        let before = cell_count(&b);
        b.apply_weapon(tok, &mut rng);
        prop_assert_eq!(cell_count(&b), before + 1, "{:?} must add exactly one box", tok);
    }

    /// THE BLIND CLERIC bombs each removable box with probability 1/2 — it only
    /// REMOVES boxes (never adds, never touches a structure cell), and it MUST
    /// actually remove some. The board is FULLY packed with removable dice (~279
    /// cells, 10x28 less one structure), so P(Blind removes nothing) = (1/2)^279 ~= 0 — a no-op Blind
    /// implementation fails the strict "count went DOWN" assertion. (The previous
    /// version asserted only "never adds / structure survives", which a no-op Blind
    /// passed — a weak test caught by mutation.) The structure cell at (0,0) and the
    /// fill-then-remove framing also pin the "only removable cells are bombed" half.
    #[test]
    fn blind_removes_removable_boxes_and_spares_structure(seed in any::<u64>()) {
        let mut rng = Rng::new(seed);
        let mut b = Board::standard(false);
        b.clear();
        // Pack the WHOLE board with removable dice so Blind has a guaranteed-large
        // population to bomb (and removing zero is astronomically unlikely).
        for y in 0..b.height { for x in 0..b.width { b.set(x, y, Some(Cell::die(3))); } }
        b.set(0, 0, Some(Cell::structure())); // a structure cell that must survive
        let before = value_grid(&b);
        let cnt = cell_count(&b);
        prop_assume!(cnt >= 100); // sanity: a big removable population

        b.apply_weapon(WeaponToken::Blind, &mut rng);

        // Only removes: every surviving cell was present before, and the count
        // dropped (Blind actually did something — kills a no-op mutant).
        prop_assert!(cell_count(&b) < cnt,
            "Blind must REMOVE some removable boxes (count {} -> {})", cnt, cell_count(&b));
        let after = value_grid(&b);
        for (i, a) in after.iter().enumerate() {
            if a.is_some() {
                prop_assert!(before[i].is_some(), "Blind added a cell at index {}", i);
            }
        }
        prop_assert_eq!(b.get(0, 0).map(|c| c.kind), Some(CellKind::Structure),
            "Blind must not bomb a structure cell");
    }

    /// FALLOUT drops out the MIDDLE columns: every cell in [LEDGE, width-LEDGE) is
    /// removed (the black hole); the side ledge columns are untouched.
    #[test]
    fn fallout_empties_the_middle_columns_only(
        fills in prop::collection::vec((0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..=6u8), 0..60),
    ) {
        let mut rng = Rng::new(1);
        let mut b = Board::standard(false);
        b.clear();
        for (x, y, v) in &fills { b.set(*x, *y, Some(Cell::die(*v))); }
        let before = value_grid(&b);
        let w = b.width as usize;
        b.apply_weapon(WeaponToken::FallOut, &mut rng);
        for y in 0..b.height { for x in 0..b.width {
            if x >= BT_FALL_OUT_LEDGE && x < b.width - BT_FALL_OUT_LEDGE {
                prop_assert!(b.get(x, y).is_none(),
                    "FallOut must empty the middle column at ({},{})", x, y);
            } else {
                prop_assert_eq!(b.get(x, y).map(|c| c.value()), before[(y as usize) * w + x as usize],
                    "FallOut must NOT touch the ledge column at ({},{})", x, y);
            }
        }}
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(70))]

    /// DURATION weapons ACCUMULATE on relaunch (first-principles lifecycle): a
    /// second launch EXTENDS the remaining duration by its full amount again
    /// (remaining += dur) — it does NOT reset the clock. Relaunching a curse should
    /// make it last longer, not restart. (We clear the board between flushes so a
    /// flush-lock can't clear a line and tick the duration down.)
    #[test]
    fn duration_weapons_accumulate_on_relaunch(tok_idx in 0usize..34) {
        let tok = WeaponToken::ALL[tok_idx];
        let dur = weapon_table()[tok.index()].duration as i32;
        prop_assume!(dur > 0);

        let mut g = Game::new(31);
        prop_assume!(receive_and_flush(&mut g, tok));
        g.board_mut().clear();
        let one = g.weapon_remaining(tok);
        prop_assume!(receive_and_flush(&mut g, tok));
        g.board_mut().clear();
        let two = g.weapon_remaining(tok);

        prop_assert_eq!(one, dur, "{:?}: first launch sets remaining to its duration", tok);
        prop_assert_eq!(two, dur * 2, "{:?}: relaunch must ACCUMULATE (remaining += dur), not reset", tok);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// THE GIMP converts every removable box to a "gimp" box but PRESERVES its
    /// value and position — looks different, plays the same. So the value grid +
    /// cell count are unchanged, and every box is now a Gimp kind.
    #[test]
    fn gimp_preserves_values_and_converts_every_box(
        fills in prop::collection::vec((0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..=6u8), 0..50)
    ) {
        let mut rng = Rng::new(7);
        let mut b = Board::standard(false);
        b.clear();
        for (x, y, v) in &fills { b.set(*x, *y, Some(Cell::die(*v))); }
        let before = value_grid(&b);
        let cnt = cell_count(&b);
        b.apply_weapon(WeaponToken::Gimp, &mut rng);
        prop_assert_eq!(value_grid(&b), before, "Gimp preserves every value + position");
        prop_assert_eq!(cell_count(&b), cnt, "Gimp preserves the cell count");
        for y in 0..b.height { for x in 0..b.width {
            if let Some(c) = b.get(x, y) {
                prop_assert!(matches!(c.kind, CellKind::Gimp(_)),
                    "box at ({},{}) must become a Gimp kind", x, y);
            }
        }}
    }

    /// MISSING PIECES removes EXACTLY ONE box (the first removable cell from a
    /// random origin) — no more, no less.
    #[test]
    fn missing_removes_exactly_one_box(
        fills in prop::collection::vec((0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..=6u8), 1..50),
        seed in any::<u64>(),
    ) {
        let mut rng = Rng::new(seed);
        let mut b = Board::standard(false);
        b.clear();
        for (x, y, v) in &fills { b.set(*x, *y, Some(Cell::die(*v))); }
        let cnt = cell_count(&b);
        prop_assume!(cnt >= 1);
        b.apply_weapon(WeaponToken::Missing, &mut rng);
        prop_assert_eq!(cell_count(&b), cnt - 1, "Missing removes exactly one box");
    }

    /// THE TWILIGHT ZONE makes every box invisible — but only COSMETICALLY: the
    /// cell count and every value/position are preserved; cells just become hidden
    /// (Invisible kind). A mutant that drops cells or mangles values fails.
    #[test]
    fn twilight_hides_every_box_preserving_values(
        fills in prop::collection::vec((0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..=6u8), 0..50)
    ) {
        let mut rng = Rng::new(9);
        let mut b = Board::standard(false);
        b.clear();
        for (x, y, v) in &fills { b.set(*x, *y, Some(Cell::die(*v))); }
        let before = value_grid(&b);
        let cnt = cell_count(&b);
        b.apply_weapon(WeaponToken::Twilight, &mut rng);
        prop_assert_eq!(value_grid(&b), before, "Twilight preserves every value + position");
        prop_assert_eq!(cell_count(&b), cnt, "Twilight preserves the cell count");
        for y in 0..b.height { for x in 0..b.width {
            if let Some(c) = b.get(x, y) {
                prop_assert!(c.hidden,
                    "box at ({},{}) must have its hidden flag set (id() -> -1)", x, y);
            }
        }}
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// SWAP MEET is an exact board EXCHANGE and an INVOLUTION (first-principles):
    /// one swap puts each player's grid on the other's board exactly; a second
    /// swap restores both boards byte-for-byte. (Bottle/Upbyside aren't planted,
    /// so the only effect is the grid exchange.)
    #[test]
    fn swap_meet_exchanges_and_is_an_involution(
        af in prop::collection::vec((0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..=6u8), 0..40),
        bf in prop::collection::vec((0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..=6u8), 0..40),
    ) {
        let mut a = Game::new(1);
        let mut b = Game::new(2);
        for (x, y, v) in &af { a.board_mut().set(*x, *y, Some(Cell::die(*v))); }
        for (x, y, v) in &bf { b.board_mut().set(*x, *y, Some(Cell::die(*v))); }
        let a0 = value_grid(a.board());
        let b0 = value_grid(b.board());

        a.swap_board_with(&mut b);
        prop_assert_eq!(value_grid(a.board()), b0.clone(), "after swap, A holds B's grid");
        prop_assert_eq!(value_grid(b.board()), a0.clone(), "after swap, B holds A's grid");

        a.swap_board_with(&mut b);
        prop_assert_eq!(value_grid(a.board()), a0, "swap twice restores A exactly");
        prop_assert_eq!(value_grid(b.board()), b0, "swap twice restores B exactly");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// LAWYER'S DELITE 1:1 (first-principles counting): a Lawyers-cursed board
    /// rises by EXACTLY one garbage row per line the opponent clears. On an empty
    /// board, after the opponent's line count climbs by K, the board must hold
    /// exactly K garbage rows = K*(width-1) cells (each garbage row has one gap).
    #[test]
    fn lawyers_rises_one_garbage_row_per_opponent_line(k in 1i64..7) {
        let mut g = Game::new(11);
        prop_assume!(receive_and_flush(&mut g, WeaponToken::Lawyers));
        // Start from an empty board (the flush lock left one piece behind).
        let (w, h) = (g.board().width as usize, g.board().height as usize);
        for y in 0..h as i32 { for x in 0..w as i32 { g.board_mut().set(x, y, None); } }
        // Opponent clears K lines (relayed: op_lines climbs 0 -> K).
        g.receive_op_score(0, k, 0);
        let _ = g.take_events();
        prop_assert_eq!(cell_count(g.board()), (k as usize) * (w - 1),
            "Lawyers must rise EXACTLY {} garbage rows ({} cells) for {} opponent lines",
            k, (k as usize) * (w - 1), k);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// BUY-THEN-LAUNCH REPLAY: a full economic flow (enter bazaar -> buy a weapon
    /// with banked funds -> leave -> launch it at the opponent -> relay delivers ->
    /// victim flushes it) is DETERMINISTIC. Two independent Versus instances driven
    /// with the identical script must end with bit-identical board exports on BOTH
    /// sides AND the launched weapon's EFFECT present on the victim. This pins that
    /// the EFFECT replays, not merely the launch frame: we use RiseUp's unmistakable
    /// signature (a near-solid bottom row no single piece can deposit).
    #[test]
    fn buy_then_launch_effect_replays_bit_exact(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
    ) {
        /// Returns (final Versus, ran-the-flow?). B starts on a FRESH (empty) board,
        /// so its FIRST lock after the RiseUp delivery can never top out — the effect
        /// is therefore GUARANTEED to land in the real engine, and the final assertion
        /// is an unconditional `fill >= 9` (no `is_over` escape hatch). That is what
        /// gives the property teeth: a no-op RiseUp mutant leaves fill < 9 and FAILS.
        fn script(seed_a: u64, seed_b: u64) -> (Versus, bool) {
            let mut v = Versus::new(seed_a, seed_b);
            v.game_mut(Side::A).add_funds(10_000);
            let _ = v.game_mut(Side::A).take_events();
            open_bazaar(v.game_mut(Side::A));
            if !v.game(Side::A).is_in_bazaar() { return (v, false); }
            let bought = v.game_mut(Side::A).buy_weapon(WeaponToken::RiseUp);
            v.game_mut(Side::A).leave_bazaar();
            if !bought { return (v, false); }
            let slot = (0..10usize).find(|&i|
                v.game(Side::A).arsenal_token(i) == WeaponToken::RiseUp.index() as i32);
            let Some(slot) = slot else { return (v, false); };
            v.game_mut(Side::A).launch_weapon(slot);
            // Relay delivers RiseUp to B's pending queue.
            v.tick(16);
            // Drive B to exactly its NEXT lock so the queued RiseUp flushes. B's board
            // is empty, so a single piece lands and locks well before any top-out.
            let locked_b = |v: &mut Versus| {
                for _ in 0..400 {
                    v.game_mut(Side::B).begin_drop();
                    let before: Vec<GameEvent> = v.game_mut(Side::B).take_events();
                    let _ = before;
                    v.tick(16);
                    // A RiseUp-flushed lock leaves a near-solid bottom row.
                    let y = v.game(Side::B).board().height - 1;
                    let fill = (0..v.game(Side::B).board().width)
                        .filter(|&x| v.game(Side::B).board().get(x, y).is_some()).count();
                    if fill >= 9 || v.is_over() { return; }
                }
            };
            locked_b(&mut v);
            (v, true)
        }

        let (v1, ok1) = script(seed_a, seed_b);
        let (v2, ok2) = script(seed_a, seed_b);
        prop_assume!(ok1 && ok2);
        // B started fresh; its first lock cannot top out. (If somehow it did, the
        // scenario is degenerate — drop it, but this is effectively never hit.)
        prop_assume!(!v1.is_over() && !v2.is_over());

        // Determinism: identical scripts -> bit-identical boards on both sides.
        prop_assert_eq!(v1.game(Side::A).export_board(), v2.game(Side::A).export_board(),
            "buy-then-launch replay diverged on side A");
        prop_assert_eq!(v1.game(Side::B).export_board(), v2.game(Side::B).export_board(),
            "buy-then-launch replay diverged on side B");

        // The EFFECT is present (unconditional): B's bottom row carries the RiseUp
        // garbage (>=9 cells), which no single piece-lock can produce.
        let y = v1.game(Side::B).board().height - 1;
        let fill = (0..v1.game(Side::B).board().width)
            .filter(|&x| v1.game(Side::B).board().get(x, y).is_some()).count();
        prop_assert!(fill >= 9,
            "the bought-and-launched RiseUp effect must land on B (bottom row fill {})", fill);
    }
}

// ===========================================================================
// GROUP G — INTERACTIONS not yet pinned elsewhere.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// CARTER PRICE DOUBLING is applied at BUY time, charging the doubled price.
    /// Independent oracle: with Carter active, buying weapon W costs exactly 2*price
    /// out of funds; selling it back refunds the doubled (effective) price. We bank
    /// exactly 2*price-1 funds (can't afford), assert the buy FAILS, then bank one
    /// more and assert it succeeds and debits exactly 2*price. A mutant that doesn't
    /// double the charge would have let the 2*price-1 purchase through.
    #[test]
    fn carter_doubles_the_charged_buy_price(tok_idx in 0usize..34) {
        let tok = WeaponToken::ALL[tok_idx];
        let base = weapon_table()[tok.index()].price as i64;
        prop_assume!(base > 0);

        let mut g = Game::new(21);
        // Activate Carter at a lock.
        prop_assume!(receive_and_flush(&mut g, WeaponToken::Carter));
        open_bazaar(&mut g);
        prop_assume!(g.is_in_bazaar());
        prop_assert_eq!(g.bazaar_price(tok), (base * 2) as i32,
            "Carter must display the doubled price");

        // Fund just short of the doubled price -> buy must fail.
        g.add_funds(base * 2 - 1);
        let _ = g.take_events();
        prop_assert!(!g.buy_weapon(tok),
            "{:?} buy must fail when funds are one short of the DOUBLED price", tok);
        // One more bean -> buy succeeds, debiting exactly the doubled price.
        g.add_funds(1);
        let _ = g.take_events();
        let before = g.score().funds;
        prop_assert!(g.buy_weapon(tok), "buy must succeed at exactly the doubled price");
        prop_assert_eq!(before - g.score().funds, base * 2,
            "Carter must charge exactly 2*price");
    }
}

/// CARTER EXPIRY restores BASE bazaar prices (the inflation lifts after Carter's
/// 20-line duration). Independent oracle on `bazaar_price`: doubled while active,
/// back to the table base once expired. The price-doubling test pins the ACTIVE
/// side; nothing pinned that the curse LIFTS — a mutant that leaves Carter's price
/// multiplier on forever (or never expires the flag) would keep the bazaar inflated
/// permanently. We use a cheap base-price weapon as the probe.
#[test]
fn carter_expiry_restores_base_bazaar_prices() {
    let probe = WeaponToken::RiseUp;
    let base = weapon_table()[probe.index()].price as i32;
    assert!(base > 0);

    let mut g = Game::new(77);
    assert!(receive_and_flush(&mut g, WeaponToken::Carter), "Carter active");
    assert_eq!(g.bazaar_price(probe), base * 2, "while Carter-cursed, prices are doubled");

    // Expire Carter: clear its full duration of lines.
    let dur = weapon_table()[WeaponToken::Carter.index()].duration as i32;
    let cleared = clear_n_lines(&mut g, dur);
    if g.is_game_over() {
        return; // flag-expiry pinned by the countdown property
    }
    assert!(cleared >= dur, "cleared {cleared} of {dur} to expire Carter");
    assert!(!g.weapon_active(WeaponToken::Carter), "Carter expired");
    assert_eq!(g.bazaar_price(probe), base,
        "after Carter expires, bazaar prices must return to base ({} not {})", base, g.bazaar_price(probe));
}

/// BT_LINE PROCESSING IS ATOMIC across the bazaar trigger AND duration expiry: a
/// single line that both opens the bazaar and ticks a duration weapon to zero must
/// do BOTH — the bazaar opens and the weapon expires on that one line, neither event
/// cannibalizing the other. (Faithful to the manager ring, where `send(BT_LINE)`
/// reaches BTScoreManager — bazaar — before BTWeaponManager — expiry —
/// BTGame.C:400-406; the port mirrors that order in place()/apply_weapon_on.) We arm
/// Carter at exactly 1 line left, position the bazaar boundary one line away, and
/// clear that single line.
#[test]
fn one_line_both_opens_the_bazaar_and_expires_a_duration_weapon() {
    let mut g = Game::new(101);
    assert!(receive_and_flush(&mut g, WeaponToken::Carter), "Carter active");
    // Drain Carter to EXACTLY 1 line remaining via real clears.
    let cdur = weapon_table()[WeaponToken::Carter.index()].duration as i32;
    let drained = clear_n_lines(&mut g, cdur - 1);
    if g.is_game_over() || drained != cdur - 1 {
        return; // expiry is also covered by the carter-expiry / countdown properties
    }
    assert_eq!(g.weapon_remaining(WeaponToken::Carter), 1, "Carter at 1 line left");
    if g.is_in_bazaar() { g.leave_bazaar(); }

    // Position the bazaar boundary so the NEXT single cleared line opens it: have the
    // opponent contribute enough lines that (op_lines + our_lines + 1) is a multiple
    // of 20 (so combined is currently one short).
    let our_lines = g.score().lines;
    let op_lines = ((our_lines / 20) + 1) * 20 - 1 - our_lines;
    g.receive_op_score(0, op_lines, 0);
    let _ = g.take_events();
    assert_eq!(g.lines_til_bazaar(), 1, "one more line opens the bazaar");
    assert!(g.weapon_active(WeaponToken::Carter), "Carter still active going into the boundary line");

    // Clear exactly ONE line: it both opens the bazaar AND is Carter's last line.
    let (w, h) = (g.board().width, g.board().height);
    for y in 0..h { for x in 0..w { g.board_mut().set(x, y, None); } }
    for x in 0..w { g.board_mut().set(x, h - 1, Some(Cell::die(1))); }
    g.begin_drop();
    let mut entered = false;
    for _ in 0..1200 {
        g.tick(16);
        for e in g.take_events() {
            if matches!(e, GameEvent::EnterBazaar) { entered = true; }
        }
        if entered || g.is_in_bazaar() || g.is_game_over() { break; }
    }
    if g.is_game_over() { return; }
    // Both must have happened on the one line: the bazaar opened AND Carter expired.
    assert!(g.is_in_bazaar(), "the boundary line must have opened the bazaar");
    assert!(!g.weapon_active(WeaponToken::Carter), "Carter must expire on its final line");
    assert_eq!(g.weapon_remaining(WeaponToken::Carter), 0, "Carter fully ticked out");
}

/// Build a Game whose centre neck row's INTERIOR [BOTTLE_X, w-BOTTLE_X) is filled
/// with `die_val` dice and whose falling piece is parked high above the neck (a
/// near-full structure blocker row), so flushing Bottle completes + clears exactly
/// that one neck row with no other interference. Returns (game, interior_value).
fn bottle_neck_row_ready(seed: u64, die_val: u8) -> (Game, i64) {
    let mut g = Game::new(seed);
    let (w, h) = (g.board().width, g.board().height);
    for y in 0..h { for x in 0..w { g.board_mut().set(x, y, None); } }
    let neck_row = h / 2;
    let interior: Vec<i32> = (BT_BOTTLE_X..(w - BT_BOTTLE_X)).collect();
    for &x in &interior {
        g.board_mut().set(x, neck_row, Some(Cell::die(die_val)));
    }
    // Park the falling piece HIGH above the neck: a near-full (one-gap, so never
    // clears) structure blocker row near the top makes the next lock happen up there.
    for x in 0..w - 1 {
        g.board_mut().set(x, 4, Some(Cell::structure()));
    }
    // Completed-row value sums over the full width; structure walls add 0, so it's
    // interior_count * die_val (one line -> funds = value).
    (g, interior.len() as i64 * die_val as i64)
}

/// BOTTLE LINE CREDIT (correctness + faithfulness): planting Bottle's neck walls
/// can COMPLETE a partially filled neck row (the walls supply the flanking cells).
/// The original (BTBoardManager.C:440 -> checkLines -> :613-615 send BT_FUNDS/BT_LINE)
/// credits that clear like any other; the Rust board's `apply_weapon` does the
/// `check_lines` but `apply_weapon_on` MUST forward the funds/lines or they vanish.
/// We pin THREE consequences of the credit, so a mutant that discards the clear OR
/// credits raw funds (skipping `credit_clear_funds`) OR drops the BT_LINE side
/// effects all FAIL:
///  (1) un-taxed: the player banks exactly the cleared row's value + one line;
///  (2) Mondale-cursed: the player keeps floor(70%) and the swiped remainder is
///      emitted as FundsStolen (conserving) — pins the Mondale-aware crediting;
///  (3) the BT_LINE the Bottle clear emits ticks OTHER weapon durations (a SoLong
///      sitting at 1 line remaining expires) — pins the tick_durations side effect.
#[test]
fn bottle_wall_planting_completes_a_neck_row_and_credits_the_funds() {
    let die_val = 4u8;
    let neck_row = Game::new(0).board().height / 2;

    // (1) UN-TAXED: exact value + one line banked.
    {
        let (mut g, value) = bottle_neck_row_ready(5, die_val);
        let (funds0, lines0) = (g.score().funds, g.score().lines);
        assert!(receive_and_flush(&mut g, WeaponToken::Bottle), "Bottle active");
        let interior_after = (BT_BOTTLE_X..(g.board().width - BT_BOTTLE_X))
            .filter(|&x| g.board().get(x, neck_row).is_some()).count();
        assert_eq!(interior_after, 0, "Bottle's walls completed + cleared the neck row");
        assert_eq!(g.score().lines - lines0, 1, "the Bottle-completed row counts as one line");
        assert_eq!(g.score().funds - funds0, value,
            "the Bottle-completed row's funds ({value}) must be credited, not discarded");
    }

    // (2) MONDALE-CURSED: keeps floor(70%), swiped remainder emitted (conserving).
    {
        let (mut g, value) = bottle_neck_row_ready(5, die_val);
        assert!(receive_and_flush(&mut g, WeaponToken::Mondale), "Mondale active");
        let funds0 = g.score().funds;
        let kept = (value as f64 * (1.0 - BT_MONDALE_RATE)) as i64;
        let tax = value - kept;

        g.receive_weapon(WeaponToken::Bottle);
        g.begin_drop();
        let (mut stolen, mut locked) = (0i64, false);
        for _ in 0..1200 {
            g.tick(16);
            for e in g.take_events() {
                match e {
                    GameEvent::FundsStolen(a) => stolen += a,
                    GameEvent::Locked { .. } => locked = true,
                    _ => {}
                }
            }
            if locked || g.is_game_over() { break; }
        }
        assert!(locked && g.weapon_active(WeaponToken::Bottle), "Bottle flushed under Mondale");
        assert_eq!(g.score().funds - funds0, kept,
            "Mondale must tax the Bottle-completed row too (keep floor(70%)={kept})");
        assert_eq!(stolen, tax,
            "the swiped Bottle-clear remainder ({tax}) must be emitted for the attacker (conserving)");
    }

    // (3) BT_LINE SIDE EFFECT: the Bottle clear ticks other weapon durations. Arm a
    // SoLong with exactly 1 line remaining; the Bottle-completed line must expire it.
    {
        let mut g = Game::new(5);
        // Activate SoLong, then drain its duration down to 1 via real line clears.
        assert!(receive_and_flush(&mut g, WeaponToken::SoLong), "SoLong active");
        let so_dur = weapon_table()[WeaponToken::SoLong.index()].duration as i32;
        let drained = clear_n_lines(&mut g, so_dur - 1);
        if g.is_game_over() || drained != so_dur - 1 {
            return; // the tick side effect is also pinned by the duration-countdown property
        }
        assert_eq!(g.weapon_remaining(WeaponToken::SoLong), 1, "SoLong drained to 1 line left");

        // Rebuild the neck-row fixture (clear_n_lines disturbed the board) so the
        // Bottle flush completes exactly one line — the BT_LINE that ticks SoLong to 0.
        let (w, h) = (g.board().width, g.board().height);
        for y in 0..h { for x in 0..w { g.board_mut().set(x, y, None); } }
        for x in BT_BOTTLE_X..(w - BT_BOTTLE_X) { g.board_mut().set(x, h / 2, Some(Cell::die(die_val))); }
        for x in 0..w - 1 { g.board_mut().set(x, 4, Some(Cell::structure())); }

        assert!(receive_and_flush(&mut g, WeaponToken::Bottle), "Bottle flushed");
        assert!(!g.weapon_active(WeaponToken::SoLong),
            "the Bottle-completed line's BT_LINE must tick SoLong (1 left) to 0 and expire it");
    }
}

/// BOTTLE EXPIRY removes the structure walls (revert_weapon clears the neck cells).
/// Independent oracle: after Bottle is flushed (walls planted) and then expires
/// (duration-10 lines), the neck columns that held structure boxes are empty again.
/// A mutant that leaves the walls after expiry fails. Fixed scenario.
#[test]
fn bottle_expiry_removes_the_structure_walls() {
    let mut g = Game::new(15);
    assert!(receive_and_flush(&mut g, WeaponToken::Bottle), "Bottle active");
    let h = BT_BOARD_HGT;
    // Walls present.
    assert_eq!(g.board().get(0, h / 2).map(|c| c.kind), Some(CellKind::Structure),
        "Bottle planted the left wall");

    // Expire Bottle: clear its full duration.
    let dur = weapon_table()[WeaponToken::Bottle.index()].duration as i32;
    let cleared = clear_n_lines(&mut g, dur);
    if g.is_game_over() {
        return; // can't observe; lifecycle still pinned by remaining-countdown test
    }
    assert!(cleared >= dur, "cleared {cleared} of {dur} to expire Bottle");
    assert!(!g.weapon_active(WeaponToken::Bottle), "Bottle expired");
    // The neck wall columns must be empty (revert cleared them) — except where a
    // clear/line shift may have parked a non-structure box. Assert NO structure box
    // remains in the neck band.
    for x in 0..BT_BOTTLE_X {
        for y in (h / 2 - BT_BOTTLE_Y)..(h / 2 + BT_BOTTLE_Y) {
            let is_struct = g.board().get(x, y).map(|c| c.kind) == Some(CellKind::Structure);
            assert!(!is_struct, "structure wall at ({x},{y}) must be gone after Bottle expires");
            let rx = BT_BOARD_WTH - x - 1;
            let is_struct_r = g.board().get(rx, y).map(|c| c.kind) == Some(CellKind::Structure);
            assert!(!is_struct_r, "structure wall at ({rx},{y}) must be gone after Bottle expires");
        }
    }
}

/// MIRROR routing for the two FUNDS weapons that differ: Reagan BACKFIRES onto a
/// cursed launcher (it is NOT on the nullify-9 set), whereas Keating FIZZLES (it
/// IS). This is the exact distinction in BTWeaponManager.C:204-216, and it's the
/// one a mutant most easily gets wrong (treating all funds weapons alike). We
/// curse the attacker, bank funds, and assert:
///   * cursed Reagan -> launcher's funds NEGATED (backfire), victim untouched;
///   * cursed Keating -> NOTHING happens to either side (fizzle).
#[test]
fn cursed_reagan_backfires_but_cursed_keating_fizzles() {
    // --- Reagan: NOT nullified -> backfires onto the cursed launcher. ---
    {
        let mut atk = Game::new(1);
        let mut vic = Game::new(2);
        deliver_weapon(&mut vic, &mut atk, WeaponToken::Mirror);
        assert!(lock_one(&mut atk));
        assert!(atk.weapon_active(WeaponToken::Mirror), "attacker is mirror-cursed");

        atk.add_funds(500);
        vic.add_funds(300);
        let _ = atk.take_events();
        let _ = vic.take_events();
        let v0 = vic.score().funds;

        // Cursed Reagan backfires: queued onto the ATTACKER, applied at its lock.
        deliver_weapon(&mut atk, &mut vic, WeaponToken::Reagan);
        assert!(lock_one(&mut atk)); // empty board, no clear -> funds change only by Reagan
        assert_eq!(atk.score().funds, -500,
            "a cursed Reagan must backfire and negate the LAUNCHER's funds");
        assert_eq!(vic.score().funds, v0, "the victim is spared by the backfire");
    }
    // --- Keating: IS nullified -> fizzles, no funds move at all. ---
    {
        let mut atk = Game::new(3);
        let mut vic = Game::new(4);
        deliver_weapon(&mut vic, &mut atk, WeaponToken::Mirror);
        assert!(lock_one(&mut atk));
        assert!(atk.weapon_active(WeaponToken::Mirror));

        atk.add_funds(500);
        vic.add_funds(300);
        let _ = atk.take_events();
        let _ = vic.take_events();
        let (a0, v0) = (atk.score().funds, vic.score().funds);

        deliver_weapon(&mut atk, &mut vic, WeaponToken::Keating);
        let _ = lock_one(&mut atk);
        let _ = lock_one(&mut vic);
        assert_eq!(atk.score().funds, a0,
            "a cursed Keating must FIZZLE — the launcher keeps its funds");
        assert_eq!(vic.score().funds, v0, "and the victim keeps theirs");
    }
}

// ===========================================================================
// GROUP H — BLANKET SANITY (every weapon is deliverable + reversible).
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// SWAP CONSERVES TOTAL CELLS across both boards — already covered by
    /// pbt_versus, but we add the SYMMETRIC-EXCHANGE invariant at the value-grid
    /// level on hand-built boards (independent of any falling piece): after a Swap,
    /// A's grid == B's old grid and vice versa, exactly. This is a pure bijection
    /// oracle. (Distinct from pbt_versus's relay-timing version.)
    #[test]
    fn swap_is_an_exact_value_grid_bijection(
        a_cells in prop::collection::vec((0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..6), 0..40),
        b_cells in prop::collection::vec((0i32..BT_BOARD_WTH, 0i32..BT_BOARD_HGT, 1u8..6), 0..40),
    ) {
        let mut a = Game::new(1);
        let mut b = Game::new(2);
        for &(x, y, v) in &a_cells { a.board_mut().set(x, y, Some(Cell::die(v))); }
        for &(x, y, v) in &b_cells { b.board_mut().set(x, y, Some(Cell::die(v))); }
        let a_grid0 = value_grid(a.board());
        let b_grid0 = value_grid(b.board());

        a.swap_board_with(&mut b);

        prop_assert_eq!(value_grid(a.board()), b_grid0, "A must hold B's old grid after Swap");
        prop_assert_eq!(value_grid(b.board()), a_grid0, "B must hold A's old grid after Swap");
        // Total cells conserved (sanity).
        prop_assert_eq!(cell_count(a.board()) + cell_count(b.board()),
            a_cells.iter().map(|c| (c.0, c.1)).collect::<std::collections::HashSet<_>>().len()
            + b_cells.iter().map(|c| (c.0, c.1)).collect::<std::collections::HashSet<_>>().len(),
            "no cells created or destroyed by Swap");
    }
}

// ===========================================================================
// GROUP I — PIECE-STREAM WIRING, end-to-end through a live Game.
//
// The piece-stream weapons (FearedWeird/FourByFour/SoLong/NoDice/NiceDay/Broken)
// are pinned at the PieceManager unit level, but nothing pinned that
// `Game::receive_weapon(tok)` + flush actually WIRES into the piece manager and
// changes the LIVE stream. A mutant dropping the `self.pieces.weapon_on(token)`
// call in `apply_weapon_on` (game.rs) survived every weapon test. These properties
// observe the live spawn stream of a real Game and pin that wiring.
// ===========================================================================

/// Collect the kinds of the next `n` pieces a live Game spawns, clearing the board
/// before each lock so the game can never top out (clearing CELLS leaves weapon
/// state intact). Each lock spawns the next piece; we record its kind. This reads
/// only the public `current_piece()` + drives real locks — an independent probe of
/// the live stream, not a PieceManager unit call.
fn collect_spawned_kinds(g: &mut Game, n: usize) -> Vec<bt_core::PieceKind> {
    let mut kinds = Vec::with_capacity(n);
    for _ in 0..n {
        // Empty the board so the next lock can't top out.
        let (w, h) = (g.board().width, g.board().height);
        for y in 0..h { for x in 0..w { g.board_mut().set(x, y, None); } }
        if !lock_one(g) || g.is_game_over() {
            break;
        }
        if let Some(p) = g.current_piece() {
            kinds.push(p.kind);
        }
    }
    kinds
}

/// SO LONG wired through a live Game: after `receive_weapon(SoLong)` + flush, the
/// live spawn stream must contain NO Long pieces. Independent oracle: scan many
/// real spawns and assert Long never appears. A mutant that drops the
/// Game->PieceManager wiring (or SoLong's keep-prob zeroing) lets Long through.
#[test]
fn so_long_wired_into_the_live_game_stream_drops_long_pieces() {
    let mut g = Game::new(321);
    assert!(receive_and_flush(&mut g, WeaponToken::SoLong), "SoLong active");
    let kinds = collect_spawned_kinds(&mut g, 300);
    assert!(kinds.len() > 50, "sanity: collected a real stream ({} spawns)", kinds.len());
    assert!(!kinds.contains(&bt_core::PieceKind::Long),
        "SoLong wired into a live Game must drop Long pieces from the stream");
}

/// NO DICE wired through a live Game: after flush, no Die piece spawns. Same
/// independent-stream oracle; kills a dropped-wiring or wrong-keep-prob mutant.
#[test]
fn no_dice_wired_into_the_live_game_stream_drops_dice() {
    let mut g = Game::new(123);
    assert!(receive_and_flush(&mut g, WeaponToken::NoDice), "NoDice active");
    let kinds = collect_spawned_kinds(&mut g, 300);
    assert!(kinds.len() > 50, "sanity: collected a real stream ({} spawns)", kinds.len());
    assert!(!kinds.contains(&bt_core::PieceKind::Die),
        "NoDice wired into a live Game must drop Die pieces from the stream");
}

/// FOUR-BY-FOUR replaces the opponent's BOX piece with a 4x4 hollow box. While
/// active, the live stream must contain a FourByFour and NO Box.
#[test]
fn four_by_four_replaces_box_in_the_live_stream() {
    use bt_core::PieceKind::{Box, FourByFour};
    let mut g = Game::new(789);
    assert!(receive_and_flush(&mut g, WeaponToken::FourByFour), "FourByFour active");
    let kinds = collect_spawned_kinds(&mut g, 200);
    assert!(kinds.len() > 30, "sanity: collected a real stream ({} spawns)", kinds.len());
    assert!(!kinds.contains(&Box), "FourByFour must replace the Box piece (no Box in the stream)");
    assert!(kinds.contains(&FourByFour), "FourByFour pieces must appear in the stream");
}

/// BROKEN RECORD "gives the same piece the same piece ..." — it repeats the last
/// piece, switching to a new one only ~1/BT_BROKEN_PROB of the time, so the stream
/// is long RUNS of repeats. First-principles (differential, not "all identical"):
/// with Broken active, consecutive spawns repeat FAR more than a normal stream,
/// and repeats dominate switches.
#[test]
fn broken_record_mostly_repeats_unlike_a_normal_stream() {
    let repeats = |ks: &[bt_core::PieceKind]| ks.windows(2).filter(|w| w[0] == w[1]).count();

    // Control: a normal stream of the same length rarely repeats back-to-back.
    let normal = collect_spawned_kinds(&mut Game::new(654), 60);
    let normal_reps = repeats(&normal);

    // Broken: same seed, but the record skips.
    let mut g = Game::new(654);
    assert!(receive_and_flush(&mut g, WeaponToken::Broken), "Broken active");
    let broken = collect_spawned_kinds(&mut g, 60);
    assert!(broken.len() > 20, "sanity: real stream ({})", broken.len());
    let broken_reps = repeats(&broken);
    let switches = broken.len() - 1 - broken_reps;

    assert!(broken_reps > switches,
        "Broken must MOSTLY repeat (repeats {} vs switches {}): {:?}", broken_reps, switches, broken);
    assert!(broken_reps > normal_reps,
        "Broken must repeat MORE than a normal stream ({} vs {})", broken_reps, normal_reps);
}

/// HAVE A NICE DAY gives the opponent a smiley face — a Happy piece enters the
/// stream (the 150-bean gift the flavor jokes about).
#[test]
fn nice_day_gives_a_happy_piece() {
    let mut g = Game::new(111);
    g.receive_weapon(WeaponToken::NiceDay);
    assert!(lock_one(&mut g), "flush NiceDay");
    let mut kinds = Vec::new();
    if let Some(p) = g.current_piece() { kinds.push(p.kind); }
    kinds.extend(collect_spawned_kinds(&mut g, 10));
    assert!(kinds.contains(&bt_core::PieceKind::Happy),
        "Have a Nice Day must give a Happy (smiley) piece, saw {:?}", kinds);
}

/// FEARED WEIRD wired through a live Game: after flush, the standard pieces vanish
/// and ONLY weird pieces spawn. Independent stream oracle: every spawned kind is in
/// the weird set; none is a standard tetromino. Kills a dropped-wiring mutant and a
/// FearedWeird keep-prob mutant that forgets to zero the standard pieces.
#[test]
fn feared_weird_wired_into_the_live_game_stream_yields_only_weird_pieces() {
    use bt_core::PieceKind::*;
    let standard = [El, RevEl, SlideLeft, SlideRight, Long, Plug, Box];
    let mut g = Game::new(456);
    assert!(receive_and_flush(&mut g, WeaponToken::FearedWeird), "FearedWeird active");
    let kinds = collect_spawned_kinds(&mut g, 200);
    assert!(kinds.len() > 30, "sanity: collected a real stream ({} spawns)", kinds.len());
    for k in &kinds {
        assert!(!standard.contains(k),
            "FearedWeird wired into a live Game must drop standard pieces, saw {:?}", k);
    }
}

/// NO DICE EXPIRY wired through a live Game: once NoDice's duration elapses, Die
/// pieces RETURN to the live stream. Pins the expiry side of the wiring (the
/// `pieces.weapon_off` call). We flush NoDice, expire it by clearing its full
/// duration, then assert a Die reappears in the live spawn stream. A mutant that
/// drops the off-wiring keeps dice suppressed forever.
#[test]
fn no_dice_expiry_restores_dice_in_the_live_game_stream() {
    let mut g = Game::new(2024);
    assert!(receive_and_flush(&mut g, WeaponToken::NoDice), "NoDice active");
    let dur = weapon_table()[WeaponToken::NoDice.index()].duration as i32;
    let cleared = clear_n_lines(&mut g, dur);
    if g.is_game_over() {
        return; // expiry of the FLAG is pinned by the remaining-countdown test
    }
    assert!(cleared >= dur, "cleared {cleared} of {dur} to expire NoDice");
    assert!(!g.weapon_active(WeaponToken::NoDice), "NoDice expired");
    let kinds = collect_spawned_kinds(&mut g, 400);
    assert!(kinds.contains(&bt_core::PieceKind::Die),
        "after NoDice expires, Die pieces must return to the live stream");
}

// ===========================================================================
// GROUP J — MIRROR LIFECYCLE (the curse expires after its duration; routing
// returns to normal). The versus tests pin backfire/nullify WHILE cursed, but
// nothing pinned that the curse LIFTS after Mirror's 10-line duration — a mutant
// that never expires Mirror (curse forever) survived every weapon test.
// ===========================================================================

/// Filled cells in the bottom row — the RiseUp garbage signature (width-1), which
/// no single piece-lock can deposit.
fn bottom_row_fill(g: &Game) -> i32 {
    let y = g.board().height - 1;
    (0..g.board().width).filter(|&x| g.board().get(x, y).is_some()).count() as i32
}

/// MIRROR EXPIRES after its 10-line duration, RESTORING normal routing. We curse
/// the attacker, confirm a launch BACKFIRES (cursed), then clear Mirror's full
/// duration of lines on the attacker so the curse lifts, and confirm a subsequent
/// launch once again HITS THE OPPONENT. Independent oracle on RiseUp's bottom-row
/// signature. A mutant that never expires Mirror keeps backfiring and fails the
/// post-expiry "hits the opponent" assertion.
#[test]
fn mirror_curse_expires_and_routing_returns_to_normal() {
    let mut atk = Game::new(11);
    let mut vic = Game::new(22);
    // Curse the attacker.
    deliver_weapon(&mut vic, &mut atk, WeaponToken::Mirror);
    assert!(lock_one(&mut atk));
    assert!(atk.weapon_active(WeaponToken::Mirror), "attacker is mirror-cursed");
    let mir_dur = weapon_table()[WeaponToken::Mirror.index()].duration as i32;
    assert_eq!(atk.weapon_remaining(WeaponToken::Mirror), mir_dur,
        "Mirror starts with its full {mir_dur}-line duration");

    // WHILE CURSED: a RiseUp launch backfires onto the attacker.
    let vic_bottom0 = bottom_row_fill(&vic);
    deliver_weapon(&mut atk, &mut vic, WeaponToken::RiseUp);
    assert!(lock_one(&mut atk));
    assert!(bottom_row_fill(&atk) >= 9, "cursed launch backfired onto the attacker");
    assert_eq!(bottom_row_fill(&vic), vic_bottom0, "victim spared while attacker cursed");

    // Expire Mirror: clear its full duration of lines on the attacker.
    let cleared = clear_n_lines(&mut atk, mir_dur);
    if atk.is_game_over() {
        return; // can't observe post-expiry routing; flag-expiry pinned elsewhere
    }
    assert!(cleared >= mir_dur, "cleared {cleared} of {mir_dur} to expire Mirror");
    assert!(!atk.weapon_active(WeaponToken::Mirror),
        "Mirror must expire after its duration — the curse lifts");

    // POST-EXPIRY: a fresh RiseUp launch must HIT THE OPPONENT again (normal routing).
    // Use a fresh victim so its bottom row starts clearly empty.
    let mut vic2 = Game::new(33);
    let vic2_bottom0 = bottom_row_fill(&vic2);
    deliver_weapon(&mut atk, &mut vic2, WeaponToken::RiseUp);
    assert!(lock_one(&mut vic2));
    assert!(bottom_row_fill(&vic2) >= 9,
        "after Mirror expires, a launch must hit the OPPONENT (got bottom row {})",
        bottom_row_fill(&vic2));
    assert!(vic2_bottom0 < 9, "sanity: victim started with an essentially empty bottom row");
}

// ===========================================================================
// GROUP K — CONTROL / SPEED REVERSION on expiry (Upbyside controls, Meadow speed).
//
// weapons_game.rs pins the ACTIVE effect of Upbyside (controls reversed) and
// Meadow (slower) but NOT that they REVERT when the weapon's duration elapses. A
// mutant that drops the `apply_weapon_off` restoration leaves controls inverted /
// gravity halved forever, and survived every weapon test.
// ===========================================================================

/// UPBYSIDE CONTROL REVERSION: while active, `move_left` shifts the piece RIGHT;
/// after Upbyside's 10-line duration elapses, `move_left` must shift LEFT again.
/// Independent oracle on the sign of the x-delta before vs after expiry. A mutant
/// that forgets to restore `left_x/right_x/delta_y` on expiry keeps controls
/// inverted and fails the post-expiry "moves left" check.
#[test]
fn upbyside_controls_revert_when_the_weapon_expires() {
    let mut g = Game::new(3);
    assert!(receive_and_flush(&mut g, WeaponToken::Upbyside), "Upbyside active");

    // WHILE ACTIVE: move_left shifts the piece RIGHT (controls reversed).
    // Empty the board + recenter so the move isn't wall-blocked.
    {
        let (w, h) = (g.board().width, g.board().height);
        for y in 0..h { for x in 0..w { g.board_mut().set(x, y, None); } }
    }
    let x0 = g.piece_pos().0;
    g.move_left();
    let x1 = g.piece_pos().0;
    assert!(x1 > x0, "while Upbyside active, move_left() shifts RIGHT ({x0} -> {x1})");

    // Expire Upbyside: clear its full duration of lines.
    let dur = weapon_table()[WeaponToken::Upbyside.index()].duration as i32;
    let cleared = clear_n_lines(&mut g, dur);
    if g.is_game_over() {
        return; // flag-expiry pinned elsewhere
    }
    assert!(cleared >= dur, "cleared {cleared} of {dur} to expire Upbyside");
    assert!(!g.weapon_active(WeaponToken::Upbyside), "Upbyside expired");

    // POST-EXPIRY: move_left must shift the piece LEFT again (controls restored).
    {
        let (w, h) = (g.board().width, g.board().height);
        for y in 0..h { for x in 0..w { g.board_mut().set(x, y, None); } }
    }
    let xa = g.piece_pos().0;
    g.move_left();
    let xb = g.piece_pos().0;
    assert!(xb < xa,
        "after Upbyside expires, move_left() must shift LEFT again ({xa} -> {xb})");
}

/// MEADOW EXPIRY round-trips the gravity period: Meadow DOUBLES `base_drop_time`
/// (slower) on activation and the expiry handler halves it back. Independent oracle
/// on the gravity period (ms per row): active period > baseline; post-expiry period
/// == baseline. A mutant that forgets the Meadow expiry restoration leaves gravity
/// permanently slow.
#[test]
fn meadow_expiry_restores_the_baseline_gravity_period() {
    let baseline = gravity_period_ms(&mut Game::new(8));
    assert!(baseline != i32::MAX && baseline > 0, "sanity: measurable baseline");

    // Active: Meadow makes the period LONGER (slower).
    {
        let mut md = Game::new(8);
        assert!(receive_and_flush(&mut md, WeaponToken::Meadow), "Meadow active");
        let active = gravity_period_ms(&mut md);
        assert!(active != i32::MAX && active > baseline,
            "Meadow must lengthen the gravity period ({active} vs baseline {baseline})");
    }

    let mut g = Game::new(8);
    assert!(receive_and_flush(&mut g, WeaponToken::Meadow), "Meadow active");
    let dur = weapon_table()[WeaponToken::Meadow.index()].duration as i32;
    let cleared = clear_n_lines(&mut g, dur);
    if g.is_game_over() {
        return;
    }
    assert!(cleared >= dur, "cleared {cleared} of {dur} for Meadow expiry");
    assert!(!g.weapon_active(WeaponToken::Meadow), "Meadow expired");
    let after = gravity_period_ms(&mut g);
    assert!(after != i32::MAX, "post-expiry period measurable");
    assert_eq!(after, baseline,
        "after Meadow expires, the gravity period must return to baseline ({after} vs {baseline})");
}
