//! Property-based tests for the export/import codec (online Swap/Susan wire format).
//!
//! Exercises:
//!   (a) BOARD round-trip: a game driven to a random state, then
//!       `import_board(&export_board())` reproduces identical boards.
//!   (b) ARSENAL round-trip: `import_arsenal(&export_arsenal())` is identity
//!       over random arsenals, crucially preserving holes (empty slots).

use bt_core::{Cell, CellKind, Game, WeaponToken};
use proptest::prelude::*;

// ---- helpers ----------------------------------------------------------------

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

// ---- (a) BOARD round-trip ---------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Drive a Game through a random op sequence, then import its board export
    /// into a fresh game and assert:
    ///   1. The re-exported bytes are bit-for-bit identical.
    ///   2. render_ids() is identical (same visual state).
    #[test]
    fn board_export_import_roundtrip(
        seed in any::<u64>(),
        ops in prop::collection::vec(op(), 0..256),
    ) {
        let mut g = Game::new(seed);
        for o in &ops {
            if g.is_game_over() {
                break;
            }
            apply(&mut g, o);
        }

        let exported = g.export_board();

        // Import into a second game seeded identically (same dimensions).
        let mut g2 = Game::new(seed);
        g2.import_board(&exported);

        let re_exported = g2.export_board();
        prop_assert_eq!(
            &exported, &re_exported,
            "re-exported bytes must match original"
        );

        // INDEPENDENT check: compare the board cell-by-cell via board().get()
        // (a DIFFERENT accessor than the export codec), so a bug in
        // export/import can't cancel out against itself. We compare the FULL
        // `Cell::encode()` (tag + value + landed + hidden), not `Cell::id()`:
        // `id()` collapses a die's pip value, a structure box, and a hidden cell
        // (all -> a render id, hidden -> -1), so a codec that lost the die VALUE,
        // the structure tag, or the hidden flag round-tripped vacuously under id().
        let (b1, b2) = (g.board(), g2.board());
        prop_assert_eq!(b1.width, b2.width);
        prop_assert_eq!(b1.height, b2.height);
        for y in 0..b1.height {
            for x in 0..b1.width {
                prop_assert_eq!(
                    b1.get(x, y).map(|c| c.encode()), b2.get(x, y).map(|c| c.encode()),
                    "board cell ({},{}) differs (encode) after import_board", x, y);
            }
        }
    }

    /// Board codec round-trip over a DIRECTLY-CONSTRUCTED board carrying the full
    /// variety of cell kinds — die pips, (un)happy faces, structure boxes, gimps,
    /// invisible cells, and HIDDEN variants. Random play (above) almost only ever
    /// produces plain color cells, so the value/landed/structure/hidden fields of
    /// the codec were never exercised there. We plant the cells explicitly and
    /// compare the round-tripped cells to the ORIGINAL `Cell`s by VALUE (`Cell`'s
    /// own `==`, which inspects the real `CellKind` — NOT via `encode()`), so a
    /// codec that drops the die value / structure tag / hidden flag can't hide
    /// the loss by zeroing it consistently on both sides of an encode-vs-encode
    /// comparison.
    #[test]
    fn board_codec_preserves_diverse_cell_kinds(
        seed in any::<u64>(),
        // a per-position kind selector for a small patch of the board
        kinds in prop::collection::vec(0u8..8, 40),
        hides in prop::collection::vec(any::<bool>(), 40),
    ) {
        // Build the patch of planted cells (kept as real `Cell`s for the oracle).
        let mut g = Game::new(seed);
        let w = g.board().width;
        let h = g.board().height;
        let mut planted: Vec<(i32, i32, Cell)> = Vec::new();
        {
            let b = g.board_mut();
            b.clear();
            for (i, (&k, &hide)) in kinds.iter().zip(hides.iter()).enumerate() {
                let x = (i as i32) % w;
                let y = h - 1 - (i as i32) / w;
                if y < 0 { break; }
                let mut cell = match k {
                    0 => Cell::color(1 + (i as i32 % 7)),
                    1 => Cell::die(1 + (i as u8 % 6)),
                    2 => Cell::happy(),
                    3 => { let mut c = Cell::happy(); c.landed(); c } // unhappy/frown
                    4 => Cell::structure(),
                    5 => Cell::gimp(3),
                    6 => Cell::new(CellKind::Invisible { id: 7, value: 11 }),
                    _ => Cell::die(6),
                };
                if hide { cell.hide(); }
                b.set(x, y, Some(cell));
                planted.push((x, y, cell));
            }
        }
        // Non-vacuity: a non-plain-color or hidden cell really is present.
        prop_assert!(
            planted.iter().any(|(_, _, c)| c.value() != 0 || c.id() == -1 || !c.is_removable()),
            "the planted board must contain a die / hidden / structure cell");

        let exported = g.export_board();
        let mut g2 = Game::new(seed.wrapping_add(1));
        g2.import_board(&exported);

        // Compare each round-tripped cell to the ORIGINAL `Cell` by value — this
        // path never calls `encode()`, so a value-dropping codec mutant is caught.
        for (x, y, original) in &planted {
            let got = g2.board().get(*x, *y);
            prop_assert_eq!(got, Some(*original),
                "cell ({},{}) changed across the board codec: planted {:?}, got {:?}",
                x, y, original, got);
            // And specifically the funds value + structure-ness survive (the parts
            // `Cell::id()` would have collapsed).
            prop_assert_eq!(got.map(|c| c.value()), Some(original.value()),
                "die/gimp VALUE lost at ({},{})", x, y);
            prop_assert_eq!(got.map(|c| c.is_removable()), Some(original.is_removable()),
                "structure-ness lost at ({},{})", x, y);
        }
    }
}

// ---- (b) ARSENAL round-trip -------------------------------------------------

/// One arsenal slot: a HOLE (-1, 0) or an occupied slot (valid token, qty).
fn holey_slot() -> impl Strategy<Value = (i32, i32)> {
    prop_oneof![
        1 => Just((-1i32, 0i32)),
        2 => ((0i32..bt_core::weapons::BT_MAX_WEAPONS as i32), 1i32..50i32),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Round-trip RANDOM arsenal layouts that deliberately include holes (empty
    /// slots between occupied ones — which `grant_weapon` would compact away, so
    /// we import a crafted layout instead). The codec must preserve the exact
    /// layout and never compact a hole.
    #[test]
    fn arsenal_codec_preserves_random_holey_layouts(
        seed in any::<u64>(),
        slots in prop::collection::vec(holey_slot(), 10),
    ) {
        // Craft a 20-int export with random holes; import_arsenal sets each slot
        // DIRECTLY (unlike grant_weapon, which fills the first empty slot).
        let mut crafted = Vec::with_capacity(20);
        for &(t, q) in &slots {
            crafted.push(t);
            crafted.push(if t < 0 { 0 } else { q });
        }
        let mut g = Game::new(seed);
        g.import_arsenal(&crafted);
        let e1 = g.export_arsenal();
        prop_assert_eq!(e1.len(), 20, "export_arsenal must return 20 ints");

        // Round-trip the (normalized) layout — must be identity.
        let mut g2 = Game::new(seed.wrapping_add(1));
        g2.import_arsenal(&e1);
        let e2 = g2.export_arsenal();
        prop_assert_eq!(&e1, &e2, "arsenal round-trip is not identity");

        // Every crafted hole stays a hole (no compaction).
        for (i, &(t, _)) in slots.iter().enumerate() {
            if t < 0 {
                prop_assert_eq!(e1[i * 2], -1,
                    "crafted-empty slot {} became occupied -> codec compacted a hole", i);
            }
        }
    }

    /// Verify holes specifically: place a weapon at slot 0, leave slot 1-8
    /// empty, place another at slot 9.  After round-trip the middle slots must
    /// still be None (no slot compaction).
    #[test]
    fn arsenal_holes_are_not_compacted(seed in any::<u64>()) {
        let mut g = Game::new(seed);
        // Grant two distinct tokens so they land in separate slots.
        g.grant_weapon(WeaponToken::RiseUp);   // slot 0
        g.grant_weapon(WeaponToken::FlipOut);  // slot 1
        // Sell slot 0 via the raw arsenal to create a hole — but sell() requires
        // in_bazaar, so we use export/import to carve the hole directly.
        //
        // Build the export manually: slot 0 = empty, slot 1 = FlipOut x1.
        let mut manual: Vec<i32> = vec![-1, 0]; // slot 0: empty
        let flipout_idx = WeaponToken::FlipOut.index() as i32;
        manual.push(flipout_idx);
        manual.push(1); // slot 1: FlipOut x1
        for _ in 2..10 {
            manual.push(-1);
            manual.push(0);
        }
        g.import_arsenal(&manual);

        let exported = g.export_arsenal();
        let mut g2 = Game::new(seed + 42);
        g2.import_arsenal(&exported);

        // Slot 0 must still be empty after round-trip.
        prop_assert_eq!(g2.arsenal_token(0), -1, "hole at slot 0 must survive round-trip");
        prop_assert_eq!(g2.arsenal_quantity(0), 0);
        // Slot 1 must still have FlipOut.
        prop_assert_eq!(g2.arsenal_token(1), flipout_idx);
        prop_assert_eq!(g2.arsenal_quantity(1), 1);
    }
}
